#[cfg(target_os = "linux")]
pub(crate) mod linux;
#[cfg(not(target_os = "linux"))]
pub(crate) mod non_linux;

use std::sync::{Arc, Mutex};
use cosmic::cosmic_config::{Config, ConfigGet};
use enigo::Enigo;
use tracing::warn;
use crate::config;

pub(crate) const LOOP_ITERATION_DELAY_MS: u64 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GlobalShortcutAction {
    RunMacro,
    StopLoop,
    #[cfg(target_os = "linux")]
    NextMacro,
    #[cfg(target_os = "linux")]
    PrevMacro,
    #[cfg(target_os = "linux")]
    ToggleLoop,
}

pub(crate) fn spawn_global_shortcut_action(
    action: GlobalShortcutAction,
    config: Config,
    enigo: Arc<Mutex<Enigo<'static>>>,
    is_looping: Arc<Mutex<bool>>,
) {
    tokio::spawn(async move {
        execute_global_shortcut_action(action, &config, &enigo, &is_looping);
    });
}

pub(crate) fn execute_global_shortcut_action(
    action: GlobalShortcutAction,
    config: &Config,
    enigo: &Arc<Mutex<Enigo<'static>>>,
    is_looping: &Arc<Mutex<bool>>,
) {
    match action {
        GlobalShortcutAction::RunMacro => run_selected_macro_from_shortcut(config, enigo, is_looping),
        GlobalShortcutAction::StopLoop => stop_macro_loop_from_shortcut(is_looping),
        #[cfg(target_os = "linux")]
        GlobalShortcutAction::NextMacro => linux::navigate_macro_from_shortcut(config, 1),
        #[cfg(target_os = "linux")]
        GlobalShortcutAction::PrevMacro => linux::navigate_macro_from_shortcut(config, -1),
        #[cfg(target_os = "linux")]
        GlobalShortcutAction::ToggleLoop => linux::toggle_loop_mode_from_shortcut(config),
    }
}

pub(crate) fn run_selected_macro_from_shortcut(
    config: &Config,
    enigo: &Arc<Mutex<Enigo<'static>>>,
    is_looping: &Arc<Mutex<bool>>,
) {
    println!("Global shortcut activated: run_macro");

    let loop_mode_enabled = config.get::<bool>("loop_mode_enabled").unwrap_or(false);
    let currently_looping = is_looping.lock().map(|state| *state).unwrap_or(false);

    if loop_mode_enabled && currently_looping {
        println!("Macro is already looping, ignoring run request");
        return;
    }
    let selected_macro_id = match config::get_selected_macro_id(config) {
        Some(id) => id,
        None => {
            println!("No macro currently selected for global shortcut");
            return;
        }
    };

    let Some(mac) = config::get_macro_by_id(config, &selected_macro_id) else {
        println!("No macro found with id {}", selected_macro_id);
        return;
    };

    let enigo = Arc::clone(enigo);

    if loop_mode_enabled {
        if let Ok(mut state) = is_looping.lock() {
            *state = true;
        }

        let loop_flag = Arc::clone(is_looping);
        tokio::task::spawn_blocking(move || {
            println!("Starting macro loop via global shortcut: {}", mac.name);
            loop {
                if let Ok(should_continue) = loop_flag.lock() {
                    if !*should_continue {
                        break;
                    }
                } else {
                    warn!("Failed to lock loop flag, stopping loop");
                    break;
                }

                mac.clone().run(Arc::clone(&enigo));
                std::thread::sleep(std::time::Duration::from_millis(LOOP_ITERATION_DELAY_MS));
            }
            println!("Macro loop stopped via global shortcut.");
        });
    } else {
        tokio::task::spawn_blocking(move || {
            println!("Running macro via global shortcut: {}", mac.name);
            mac.run(enigo);
            println!("Macro complete.");
        });
    }
}

pub(crate) fn stop_macro_loop_from_shortcut(is_looping: &Arc<Mutex<bool>>) {
    println!("Global shortcut activated: stop_loop");
    if let Ok(mut state) = is_looping.lock() {
        *state = false;
        println!("Loop stop requested via global shortcut.");
    } else {
        println!("Failed to access loop flag.");
    }
}
