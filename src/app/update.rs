#[cfg(not(target_os = "linux"))]
use crate::app::hotkeys::{spawn_global_shortcut_action, GlobalShortcutAction};
use crate::app::key_mapping::{map_iced_key_to_enigo_key, map_iced_physical_key_to_enigo_key};
use crate::app::message::Message;
use crate::app::message::Message::*;
use crate::app::view::CLEAR_CONFIRM_TIMEOUT_SECS;
use crate::app::App;
use crate::config::{self, get_macros_from_config, save_config_value, set_selected_macro_id};
use crate::macros::loop_control;
use crate::macros::{thread, Instruction};
use cosmic::app::Task;
use cosmic::cosmic_config::ConfigGet;
use cosmic::iced::keyboard;
use enigo::agent::Token;
#[cfg(not(target_os = "linux"))]
use global_hotkey::HotKeyState;
use std::sync::Arc;
use tracing::warn;

fn push_undo(app: &mut App) {
    if let Some(mac) = &app.macro_lib.current_macro {
        if app.editor_ui.undo_stack.len() >= 50 {
            app.editor_ui.undo_stack.remove(0);
        }
        app.editor_ui.undo_stack.push(mac.code.clone());
        app.editor_ui.redo_stack.clear();
    }
}

pub(crate) fn handle_update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        SetTitle(title) => {
            if let Some(mac) = &mut app.macro_lib.current_macro {
                mac.name = title;
                auto_save_current_macro(app);
            }
        }
        SetDescription(desc) => {
            if let Some(mac) = &mut app.macro_lib.current_macro {
                mac.description = desc;
                auto_save_current_macro(app);
            }
        }
        SelectMacro(selected) => {
            let config = app.config.clone();
            app.macro_lib.update_macro(&config, Some(selected));
            app.editor_ui.undo_stack.clear();
            app.editor_ui.redo_stack.clear();
        }
        Undo => {
            if let Some(prev_code) = app.editor_ui.undo_stack.pop() {
                let current_code = app.macro_lib.current_macro.as_ref().map(|m| m.code.clone());
                if let Some(current) = current_code {
                    app.editor_ui.redo_stack.push(current);
                }
                if let Some(mac) = &mut app.macro_lib.current_macro {
                    mac.code = prev_code;
                }
                auto_save_current_macro(app);
            }
        }
        Redo => {
            if let Some(next_code) = app.editor_ui.redo_stack.pop() {
                let current_code = app.macro_lib.current_macro.as_ref().map(|m| m.code.clone());
                if let Some(current) = current_code {
                    app.editor_ui.undo_stack.push(current);
                }
                if let Some(mac) = &mut app.macro_lib.current_macro {
                    mac.code = next_code;
                }
                auto_save_current_macro(app);
            }
        }
        RunMacro => {
            if let Some(mac) = app.macro_lib.current_macro.clone() {
                if app.execution.loop_mode_enabled {
                    if let Err(err) = loop_control::start_loop(&app.execution.is_looping) {
                        warn!("Failed to start loop: {}", err);
                        return Task::none();
                    }

                    let loop_task = mac.clone().into_loop_task(
                        Arc::clone(&app.execution.enigo),
                        Arc::clone(&app.execution.is_looping),
                    );

                    if let Err(err) = thread::spawn_macro_thread(
                        &mut app.execution.thread_pool,
                        format!("loop_{}", mac.name),
                        loop_task,
                    ) {
                        warn!("Failed to spawn loop thread: {}", err);
                        let _ = loop_control::stop_loop(&app.execution.is_looping);
                    }
                } else {
                    let single_task = mac.clone().into_single_run_task(
                        Arc::clone(&app.execution.enigo),
                    );

                    if let Err(err) = thread::spawn_macro_thread(
                        &mut app.execution.thread_pool,
                        format!("single_{}", mac.name),
                        single_task,
                    ) {
                        warn!("Failed to spawn single run thread: {}", err);
                    }
                }
            }
        }
        AddInstruction(index, instruction) => {
            push_undo(app);
            if let Some(mac) = &mut app.macro_lib.current_macro {
                mac.code.insert(index, instruction);
                auto_save_current_macro(app);
            }
        }
        EditInstruction(index, instruction) => {
            if let Some(mac) = &mut app.macro_lib.current_macro {
                if !mac.code.is_empty() {
                    mac.code[index] = instruction;
                    auto_save_current_macro(app);
                }
            }
        }
        StartKeyCapture(index) => {
            app.editor_ui.key_capture_index = Some(index);
        }
        KeyCaptureEvent(event) => {
            if let keyboard::Event::KeyPressed { key, physical_key, .. } = event {
                let Some(index) = app.editor_ui.key_capture_index else {
                    return Task::none();
                };

                if let Some(mac) = &mut app.macro_lib.current_macro {
                    if let Some(Instruction::Token(Token::Key(_, direction))) =
                        mac.code.get(index).cloned()
                    {
                        let captured_key = map_iced_physical_key_to_enigo_key(physical_key)
                            .or_else(|| map_iced_key_to_enigo_key(key.as_ref()));

                        if let Some(captured_key) = captured_key {
                            mac.code[index] = Instruction::Token(Token::Key(captured_key, direction));
                            auto_save_current_macro(app);
                        }
                    }
                }

                app.editor_ui.key_capture_index = None;
            }
        }
        RemoveInstruction(index) => {
            if let Some(mac) = &app.macro_lib.current_macro {
                if !mac.code.is_empty() && index >= 0 {
                    push_undo(app);
                    if let Some(mac) = &mut app.macro_lib.current_macro {
                        mac.code.remove(index as usize);
                        auto_save_current_macro(app);
                    }
                }
            }
        }
        ReorderInstruction(index, direction) => {
            if let Some(mac) = &app.macro_lib.current_macro {
                let len = mac.code.len();
                if len > 1 && index < len {
                    let new_index = if direction < 0 {
                        if index > 0 { index - 1 } else { index }
                    } else {
                        if index < len - 1 { index + 1 } else { index }
                    };
                    if new_index != index {
                        push_undo(app);
                        if let Some(mac) = &mut app.macro_lib.current_macro {
                            mac.code.swap(index, new_index);
                            auto_save_current_macro(app);
                        }
                    }
                }
            }
        }
        ClearInstructions => {
            if !app.editor_ui.confirm_clear_instructions {
                app.editor_ui.confirm_clear_instructions = true;
                app.editor_ui.clear_confirm_generation =
                    app.editor_ui.clear_confirm_generation.wrapping_add(1);
                let generation = app.editor_ui.clear_confirm_generation;
                return Task::perform(
                    async move {
                        tokio::time::sleep(std::time::Duration::from_secs(CLEAR_CONFIRM_TIMEOUT_SECS)).await;
                        generation
                    },
                    |generation| ClearInstructionsTimeout(generation).into(),
                );
            } else {
                push_undo(app);
                if let Some(mac) = &mut app.macro_lib.current_macro {
                    mac.code.clear();
                    auto_save_current_macro(app);
                    app.editor_ui.confirm_clear_instructions = false;
                }
            }
        }
        ClearInstructionsTimeout(generation) => {
            if generation == app.editor_ui.clear_confirm_generation {
                app.editor_ui.confirm_clear_instructions = false;
            }
        }
        SaveMacro => {
            if let Some(mac) = &app.macro_lib.current_macro {
                if let Err(err) = mac.save() {
                    warn!("Failed to save macro: {}", err);
                } else {
                    let config = app.config.clone();
                    app.macro_lib.update_macros(&config);
                }
            }
        }
        NewMacro => {
            use crate::macros::Macro;
            let new_macro = Macro::new("New Macro".into(), "New Macro".into(), vec![]);
            let new_id = new_macro.id.clone();
            if let Err(err) = new_macro.add() {
                warn!("Failed to create macro: {}", err);
            }
            let config = app.config.clone();
            app.macro_lib.update_macros(&config);
            if let Some((index, _)) = config::get_macros_from_config(&app.config)
                .iter()
                .enumerate()
                .find(|(_, mac)| mac.id == new_id)
            {
                let config = app.config.clone();
                app.macro_lib.update_macro(&config, Some(index));
            }
        }
        RemoveMacro => {
            if !app.editor_ui.confirm_remove_macro {
                app.editor_ui.confirm_remove_macro = true;
            } else {
                if let Some(mac) = app.macro_lib.current_macro.clone() {
                    if let Err(err) = mac.clone().remove() {
                        warn!("Failed to remove macro: {}", err);
                    } else {
                        let config = app.config.clone();
                        app.macro_lib.update_macros(&config);
                        let config = app.config.clone();
                        app.macro_lib.update_macro(&config, None);
                    }
                }
                app.editor_ui.confirm_remove_macro = false;
            }
        }
        ToggleLoopMode(enabled) => {
            app.execution.loop_mode_enabled = enabled;
            if let Err(err) = config::save_config_value(&app.config, "loop_mode_enabled", enabled) {
                warn!("Failed to save loop mode setting: {}", err);
            }
        }
        #[cfg(not(target_os = "linux"))]
        GlobalHotkeyEvent(event) => {
            if event.state != HotKeyState::Pressed {
                return Task::none();
            }

            println!("{:?}", event);
            let action = if event.id == app.hotkey_state.run_macro_id {
                Some(GlobalShortcutAction::RunMacro)
            } else if event.id == app.hotkey_state.stop_loop_id {
                Some(GlobalShortcutAction::StopLoop)
            } else {
                None
            };

            if let Some(action) = action {
                spawn_global_shortcut_action(
                    action,
                    app.config.clone(),
                    Arc::clone(&app.execution.enigo),
                    Arc::clone(&app.execution.is_looping),
                );
            }
        }
    }
    Task::none()
}

pub(crate) fn auto_save_current_macro(app: &mut App) {
    if let Some(mac) = &app.macro_lib.current_macro {
        if let Err(err) = mac.save() {
            warn!("Failed to update macro: {}", err);
        } else {
            if let Err(err) = config::set_selected_macro_id(&app.config, Some(&mac.id)) {
                warn!("Failed to save selected macro id: {}", err);
            }
            let config = app.config.clone();
            app.macro_lib.update_macros(&config);
        }
    }
}
