#![cfg(target_os = "linux")]

use super::{spawn_global_shortcut_action, GlobalShortcutAction};
use crate::app::message::Message;
use crate::config;
use crate::recording;
use cosmic::cosmic_config::{Config, ConfigGet};
use cosmic::iced::futures::{SinkExt, Stream, StreamExt};
use enigo::Enigo;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::sleep;

static MACRO_NAV_QUEUE: OnceLock<Mutex<VecDeque<usize>>> = OnceLock::new();

pub(crate) fn get_macro_nav_queue() -> &'static Mutex<VecDeque<usize>> {
    MACRO_NAV_QUEUE.get_or_init(|| Mutex::new(VecDeque::new()))
}

static LOOP_TOGGLE_QUEUE: OnceLock<Mutex<VecDeque<bool>>> = OnceLock::new();

pub(crate) fn get_loop_toggle_queue() -> &'static Mutex<VecDeque<bool>> {
    LOOP_TOGGLE_QUEUE.get_or_init(|| Mutex::new(VecDeque::new()))
}

pub(crate) fn macro_nav_sub() -> impl Stream<Item = Message> {
    cosmic::iced::stream::channel(32, |mut sender: cosmic::iced::futures::channel::mpsc::Sender<Message>| async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            let pending_nav: Vec<usize> = get_macro_nav_queue()
                .try_lock()
                .map(|mut q| q.drain(..).collect())
                .unwrap_or_default();
            for idx in pending_nav {
                let _ = sender.send(Message::SelectMacro(idx)).await;
            }
            let pending_toggles: Vec<bool> = get_loop_toggle_queue()
                .try_lock()
                .map(|mut q| q.drain(..).collect())
                .unwrap_or_default();
            for enabled in pending_toggles {
                let _ = sender.send(Message::ToggleLoopMode(enabled)).await;
            }
            let stop_count = recording::get_stop_signal()
                .try_lock()
                .map(|mut q| q.drain(..).count())
                .unwrap_or(0);
            for _ in 0..stop_count {
                let _ = sender.send(Message::StopRecording).await;
            }
        }
    })
}

pub(crate) fn navigate_macro_from_shortcut(config: &Config, direction: isize) {
    let macros = config::get_macros_from_config(config);
    if macros.is_empty() { return; }
    let current_id = config::get_selected_macro_id(config);
    let current_idx = current_id
        .and_then(|id| macros.iter().position(|m| m.id == id))
        .unwrap_or(0);
    let len = macros.len();
    let next_idx = if direction > 0 {
        (current_idx + 1) % len
    } else if current_idx == 0 {
        len - 1
    } else {
        current_idx - 1
    };
    if let Ok(mut q) = get_macro_nav_queue().lock() {
        q.push_back(next_idx);
    }
}

pub(crate) fn toggle_loop_mode_from_shortcut(config: &Config) {
    let current = config.get::<bool>("loop_mode_enabled").unwrap_or(false);
    if let Ok(mut q) = get_loop_toggle_queue().lock() {
        q.push_back(!current);
    }
}

pub(crate) fn setup_linux_hotkeys(
    enigo: Arc<Mutex<Enigo<'static>>>,
    config: Config,
    is_looping: Arc<Mutex<bool>>,
) {
    use cosmic::dialog::ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
    use cosmic::iced::futures::executor::block_on;
    use tracing::warn;

    if let Ok(shortcuts) = block_on(GlobalShortcuts::new()) {
        if let Ok(session) = block_on(shortcuts.create_session()) {
            let run_macro_sc = NewShortcut::new("run_macro", "Run Current Macro")
                .preferred_trigger(Some("<Ctrl><Alt>M"));
            let stop_loop_sc = NewShortcut::new("stop_loop", "Stop Macro Loop")
                .preferred_trigger(Some("<Ctrl><Alt>S"));
            let next_macro_sc = NewShortcut::new("next_macro", "Select Next Macro")
                .preferred_trigger(Some("<Ctrl><Alt>Right"));
            let prev_macro_sc = NewShortcut::new("prev_macro", "Select Previous Macro")
                .preferred_trigger(Some("<Ctrl><Alt>Left"));
            let toggle_loop_sc = NewShortcut::new("toggle_loop_mode", "Toggle Loop Mode")
                .preferred_trigger(Some("<Ctrl><Alt>L"));

            if block_on(shortcuts.bind_shortcuts(&session, &[run_macro_sc, stop_loop_sc, next_macro_sc, prev_macro_sc, toggle_loop_sc], None)).is_ok() {
                if let Ok(mut activations) = block_on(shortcuts.receive_activated()) {
                    let enigo_clone = Arc::clone(&enigo);
                    let config_clone = config.clone();
                    let is_looping_clone = Arc::clone(&is_looping);

                    tokio::spawn(async move {
                        while let Some(evt) = activations.next().await {
                            match evt.shortcut_id() {
                                "run_macro" => {
                                    spawn_global_shortcut_action(
                                        GlobalShortcutAction::RunMacro,
                                        config_clone.clone(),
                                        Arc::clone(&enigo_clone),
                                        Arc::clone(&is_looping_clone),
                                    );
                                }
                                "stop_loop" => {
                                    spawn_global_shortcut_action(
                                        GlobalShortcutAction::StopLoop,
                                        config_clone.clone(),
                                        Arc::clone(&enigo_clone),
                                        Arc::clone(&is_looping_clone),
                                    );
                                }
                                "next_macro" => {
                                    spawn_global_shortcut_action(
                                        GlobalShortcutAction::NextMacro,
                                        config_clone.clone(),
                                        Arc::clone(&enigo_clone),
                                        Arc::clone(&is_looping_clone),
                                    );
                                }
                                "prev_macro" => {
                                    spawn_global_shortcut_action(
                                        GlobalShortcutAction::PrevMacro,
                                        config_clone.clone(),
                                        Arc::clone(&enigo_clone),
                                        Arc::clone(&is_looping_clone),
                                    );
                                }
                                "toggle_loop_mode" => {
                                    spawn_global_shortcut_action(
                                        GlobalShortcutAction::ToggleLoop,
                                        config_clone.clone(),
                                        Arc::clone(&enigo_clone),
                                        Arc::clone(&is_looping_clone),
                                    );
                                }
                                _ => {}
                            }
                        }
                    });
                } else {
                    warn!("Global shortcuts unavailable: failed to receive activations stream");
                }
            } else {
                warn!("Global shortcuts unavailable: failed to bind shortcuts");
            }
        } else {
            warn!("Global shortcuts unavailable: failed to create session");
        }
    } else {
        warn!("Global shortcuts unavailable: failed to initialize shortcuts");
    }
}
