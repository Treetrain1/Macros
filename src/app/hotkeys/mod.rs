pub(crate) mod queue;

use crate::config;
use crate::hotkey_types::HotkeyAction;
use crate::macros::backend::InputBackend;
use cosmic::cosmic_config::{Config, ConfigGet};
use std::sync::{Arc, Mutex};
use tracing::warn;

pub(crate) const LOOP_ITERATION_DELAY_MS: u64 = 1;

pub(crate) fn spawn_hotkey_action(
    action: HotkeyAction,
    config: Config,
    emulator: Arc<Mutex<dyn InputBackend>>,
    is_looping: Arc<Mutex<bool>>,
) {
    tokio::spawn(async move {
        execute_run_action(action, &config, &emulator, &is_looping);
    });
}

fn execute_run_action(
    action: HotkeyAction,
    config: &Config,
    emulator: &Arc<Mutex<dyn InputBackend>>,
    is_looping: &Arc<Mutex<bool>>,
) {
    match action {
        HotkeyAction::RunMacro => run_selected_macro(config, emulator, is_looping),
        HotkeyAction::RunSpecificMacro(ref macro_id) => {
            run_specific_macro(config, emulator, is_looping, macro_id)
        }
        _ => {}
    }
}

pub(crate) fn run_selected_macro(
    config: &Config,
    emulator: &Arc<Mutex<dyn InputBackend>>,
    is_looping: &Arc<Mutex<bool>>,
) {
    let loop_mode_enabled = config.get::<bool>("loop_mode_enabled").unwrap_or(false);
    let currently_looping = is_looping.lock().map(|s| *s).unwrap_or(false);

    if loop_mode_enabled && currently_looping {
        return;
    }

    let selected_id = match config::get_selected_macro_id(config) {
        Some(id) => id,
        None => return,
    };

    let Some(mac) = config::get_macro_by_id(config, &selected_id) else {
        return;
    };

    run_macro_task(mac, emulator, is_looping, loop_mode_enabled);
}

pub(crate) fn run_specific_macro(
    config: &Config,
    emulator: &Arc<Mutex<dyn InputBackend>>,
    is_looping: &Arc<Mutex<bool>>,
    macro_id: &str,
) {
    let loop_mode_enabled = config.get::<bool>("loop_mode_enabled").unwrap_or(false);
    let currently_looping = is_looping.lock().map(|s| *s).unwrap_or(false);

    if loop_mode_enabled && currently_looping {
        return;
    }

    let Some(mac) = config::get_macro_by_id(config, macro_id) else {
        return;
    };

    run_macro_task(mac, emulator, is_looping, loop_mode_enabled);
}

fn run_macro_task(
    mac: crate::macros::Macro,
    emulator: &Arc<Mutex<dyn InputBackend>>,
    is_looping: &Arc<Mutex<bool>>,
    loop_mode: bool,
) {
    let emulator = Arc::clone(emulator);

    if loop_mode {
        if let Ok(mut state) = is_looping.lock() {
            *state = true;
        }
        let loop_flag = Arc::clone(is_looping);
        tokio::task::spawn_blocking(move || {
            #[cfg(windows)]
            crate::macros::backend::windows::prepare_for_macro_execution();
            loop {
                if let Ok(should_continue) = loop_flag.lock() {
                    if !*should_continue {
                        break;
                    }
                } else {
                    warn!("Failed to lock loop flag, stopping");
                    break;
                }
                mac.clone().run(Arc::clone(&emulator), Some(Arc::clone(&loop_flag)));
                std::thread::sleep(std::time::Duration::from_millis(LOOP_ITERATION_DELAY_MS));
            }
        });
    } else {
        if let Ok(mut state) = is_looping.lock() {
            *state = true;
        }
        let stop_flag = Arc::clone(is_looping);
        tokio::task::spawn_blocking(move || {
            #[cfg(windows)]
            crate::macros::backend::windows::prepare_for_macro_execution();
            mac.run(emulator, Some(Arc::clone(&stop_flag)));
            if let Ok(mut state) = stop_flag.lock() {
                *state = false;
            }
        });
    }
}

pub(crate) fn stop_loop(is_looping: &Arc<Mutex<bool>>) {
    if let Ok(mut state) = is_looping.lock() {
        *state = false;
    }
}
