pub(crate) mod commands;
pub(crate) mod config;
pub(crate) mod hotkey_types;
pub(crate) mod input;
pub(crate) mod ipc;
pub(crate) mod key_mapping;
pub(crate) mod macros;
pub(crate) mod recording;
pub(crate) mod state;
#[cfg(windows)]
pub(crate) mod updater;

use crate::macros::runner::make_backend;
use crate::macros::thread_pool::ThreadPool;
use crate::recording::QueueSignal;
use crate::state::{AppState, ComboCapture, Page, RecordingPhase, SharedState, UpdateCheckState};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{Cef, Manager};

pub fn run() {
    tracing_subscriber::fmt::init();
    let _ = tracing_log::LogTracer::init();

    tauri::Builder::<Cef>::default()
        .command_line_args([
            ("--ozone-platform".to_string(), Some("x11".to_string())),
        ])
        .setup(|app| {
            let settings = config::load_settings();

            let initial_state = AppState {
                macro_selected: None,
                current_macro: None,
                macros_list: vec![],
                macro_strs: vec![],
                emulator: make_backend(),
                thread_pool: ThreadPool::new(),
                is_looping: Arc::new(Mutex::new(false)),
                loop_mode_enabled: settings.loop_mode_enabled.unwrap_or(false),
                ipc_server: None,
                ipc_shutdown_tx: None,
                ipc_active_port: None,
                ipc_auto_start: settings.ipc_auto_start.unwrap_or(false),
                confirm_remove_macro: false,
                confirm_clear_instructions: false,
                clear_confirm_generation: 0,
                key_capture_index: None,
                undo_stack: vec![],
                redo_stack: vec![],
                recording_phase: RecordingPhase::Idle,
                recording_countdown_generation: 0,
                record_mouse_relative: true,
                page: Page::Main,
                combo_capture: None,
                hotkey_bindings: vec![],
                pending_macro_hotkey: None,
                invalid_field_buffers: HashMap::new(),
                ipc_port_text: settings.ipc_port.unwrap_or(47821).to_string(),
                ipc_port_invalid: false,
                update_check_state: UpdateCheckState::Idle,
            };

            let shared: SharedState = Arc::new(Mutex::new(initial_state));
            app.manage(shared.clone());

            // ── Startup setup ──────────────────────────────────────────────
            {
                let mut s = shared.lock().unwrap();

                // Load macros; create a default one if empty
                let macros = config::get_macros_from_config();
                if macros.is_empty() {
                    let _ = crate::macros::Macro::new("New Macro".into(), "".into(), vec![]).add();
                }
                let macros = config::get_macros_from_config();
                s.macro_strs = macros.iter().map(|m| m.name.clone()).collect();

                // Restore selection
                if let Some(ref id) = settings.selected_macro_id {
                    if let Some((idx, mac)) = macros.iter().enumerate().find(|(_, m)| &m.id == id) {
                        s.macro_selected = Some(idx);
                        s.current_macro = Some(mac.clone());
                    }
                }
                s.macros_list = macros;

                // macOS accessibility
                #[cfg(target_os = "macos")]
                {
                    let trusted = crate::macros::backend::macos::request_accessibility();
                    if !trusted {
                        recording::set_grab_failed(true);
                    }
                }

                recording::start_grab_thread();

                let bindings = config::load_hotkey_bindings();
                recording::update_hotkey_table(bindings.clone());
                s.hotkey_bindings = bindings;

                // Auto-start IPC server if configured
                if s.ipc_auto_start {
                    if let Ok(port) = s.ipc_port_text.trim().parse::<u16>() {
                        let (tx, rx) = tokio::sync::watch::channel(false);
                        s.ipc_server = Some(tokio::spawn(crate::ipc::run_server(port, rx)));
                        s.ipc_shutdown_tx = Some(tx);
                        s.ipc_active_port = Some(port);
                    }
                }
            }

            // ── QueueSignal consumer (replaces iced hotkey subscription) ──
            let app_handle = app.handle().clone();
            let state_for_task = Arc::clone(&shared);
            tauri::async_runtime::spawn(async move {
                let mut rx = recording::take_queue_receiver();
                while let Some(signal) = rx.recv().await {
                    match signal {
                        QueueSignal::Hotkey(action) => {
                            commands::handle_hotkey_action(&state_for_task, &app_handle, action);
                        }
                        QueueSignal::Stop => {
                            commands::stop_recording_internal(&state_for_task, &app_handle);
                        }
                    }
                }
            });

            // ── Delayed update check (Windows only) ────────────────────────
            #[cfg(windows)]
            {
                let state_w = Arc::clone(&shared);
                let app_w = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    commands::check_for_updates_internal(&state_w, &app_w).await;
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::select_macro,
            commands::new_macro,
            commands::remove_macro,
            commands::set_title,
            commands::save_macro,
            commands::add_instruction,
            commands::edit_instruction,
            commands::edit_instruction_field,
            commands::remove_instruction,
            commands::reorder_instruction,
            commands::clear_instructions,
            commands::undo,
            commands::redo,
            commands::start_key_capture,
            commands::key_capture_event,
            commands::run_macro,
            commands::toggle_loop_mode,
            commands::start_recording,
            commands::stop_recording,
            commands::toggle_record_mouse_relative,
            commands::open_settings,
            commands::close_settings,
            commands::start_combo_capture,
            commands::start_pending_combo_capture,
            commands::combo_capture_event,
            commands::cancel_combo_capture,
            commands::set_pending_macro_idx,
            commands::add_macro_hotkey,
            commands::remove_hotkey_binding,
            commands::clear_named_hotkey,
            commands::reset_hotkey_to_default,
            commands::set_ipc_port_text,
            commands::start_ipc_server,
            commands::stop_ipc_server,
            commands::set_ipc_auto_start,
            commands::check_for_updates,
            commands::apply_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
