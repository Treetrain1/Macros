use crate::app::hotkeys;
use crate::app::key_mapping::{
    iced_code_to_rdev_key_name, is_modifier_code, map_iced_key_to_macro_key,
    map_iced_physical_key_to_macro_key, mods_to_u8,
};
use crate::app::message::Message;
use crate::app::message::Message::*;
use crate::app::state::{ComboCapture, FieldId, Page, RecordingPhase, UpdateCheckState};
use crate::app::view::CLEAR_CONFIRM_TIMEOUT_SECS;
use crate::app::App;
use crate::config::{self, get_macros_from_config, save_config_value, set_selected_macro_id};
use crate::hotkey_types::{HotkeyBinding, HotkeyAction, KeyCombo};
use crate::macros::loop_control;
use crate::macros::{thread, Instruction};
use crate::recording;
use cosmic::app::Task;
use cosmic::cosmic_config::ConfigGet;
use cosmic::iced::keyboard;
use cosmic::iced::widget::scrollable;
use crate::input::types::InputToken;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tracing::warn;

fn scroll_editor_to_end() -> Task<Message> {
    scrollable::snap_to(
        cosmic::widget::Id::new("macro-editor-scroll"),
        scrollable::RelativeOffset::END.into(),
    )
}

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
            app.editor_ui.invalid_field_buffers.clear();
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
                app.editor_ui.invalid_field_buffers.clear();
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
                app.editor_ui.invalid_field_buffers.clear();
                auto_save_current_macro(app);
            }
        }
        RunMacro => {
            if let (Some(mac), Some(emulator)) = (
                app.macro_lib.current_macro.clone(),
                app.execution.emulator.as_ref(),
            ) {
                if app.execution.loop_mode_enabled {
                    if let Err(err) = loop_control::start_loop(&app.execution.is_looping) {
                        warn!("Failed to start loop: {}", err);
                        return Task::none();
                    }

                    let loop_task = mac.clone().into_loop_task(
                        Arc::clone(emulator),
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
                    let _ = loop_control::set_loop_state(&app.execution.is_looping, true);
                    let single_task = mac.clone().into_single_run_task(
                        Arc::clone(emulator),
                        Arc::clone(&app.execution.is_looping),
                    );

                    if let Err(err) = thread::spawn_macro_thread(
                        &mut app.execution.thread_pool,
                        format!("single_{}", mac.name),
                        single_task,
                    ) {
                        warn!("Failed to spawn single run thread: {}", err);
                        let _ = loop_control::set_loop_state(&app.execution.is_looping, false);
                    }
                }
            }
        }
        AddInstruction(index, instruction) => {
            push_undo(app);
            if let Some(mac) = &mut app.macro_lib.current_macro {
                let is_append = index == mac.code.len();
                mac.code.insert(index, instruction);
                app.editor_ui.invalid_field_buffers.clear();
                auto_save_current_macro(app);
                if is_append {
                    return scroll_editor_to_end();
                }
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
        EditInstructionField(index, field, text) => {
            if let Some(mac) = &mut app.macro_lib.current_macro {
                if let Some(current) = mac.code.get(index).cloned() {
                    let parsed_ok = match (&current, field) {
                        (Instruction::Wait(_, randomness), FieldId::WaitDuration) => {
                            match text.parse::<f64>() {
                                Ok(v) => {
                                    mac.code[index] = Instruction::Wait(v, *randomness);
                                    true
                                }
                                Err(_) => false,
                            }
                        }
                        (Instruction::Wait(duration, _), FieldId::WaitRandomness) => {
                            match text.parse::<f64>() {
                                Ok(v) => {
                                    mac.code[index] = Instruction::Wait(*duration, v);
                                    true
                                }
                                Err(_) => false,
                            }
                        }
                        (Instruction::Token(InputToken::MoveMouse(_, y, coord)), FieldId::MoveMouseX) => {
                            match text.parse::<i32>() {
                                Ok(v) => {
                                    mac.code[index] = Instruction::Token(InputToken::MoveMouse(v, *y, coord.clone()));
                                    true
                                }
                                Err(_) => false,
                            }
                        }
                        (Instruction::Token(InputToken::MoveMouse(x, _, coord)), FieldId::MoveMouseY) => {
                            match text.parse::<i32>() {
                                Ok(v) => {
                                    mac.code[index] = Instruction::Token(InputToken::MoveMouse(*x, v, coord.clone()));
                                    true
                                }
                                Err(_) => false,
                            }
                        }
                        (Instruction::Token(InputToken::Scroll(_, axis)), FieldId::ScrollAmount) => {
                            match text.parse::<i32>() {
                                Ok(v) => {
                                    mac.code[index] = Instruction::Token(InputToken::Scroll(v, axis.clone()));
                                    true
                                }
                                Err(_) => false,
                            }
                        }
                        _ => false,
                    };

                    // Always keep the literally-typed text on screen (even once it parses
                    // successfully) so reformatting the committed value doesn't make
                    // characters like a trailing "." appear to vanish while typing.
                    app.editor_ui.invalid_field_buffers.insert((index, field), text);
                    if parsed_ok {
                        auto_save_current_macro(app);
                    }
                }
            }
        }
        StartKeyCapture(index) => {
            app.editor_ui.key_capture_index = Some(index);
        }
        KeyCaptureEvent(event) => {
            // Combo capture for hotkey settings
            if app.editor_ui.combo_capture.is_some() {
                handle_combo_capture_event(app, event);
                return Task::none();
            }

            // Key capture for instruction editing
            if let keyboard::Event::KeyPressed {
                key,
                physical_key,
                ..
            } = event
            {
                let Some(index) = app.editor_ui.key_capture_index else {
                    return Task::none();
                };

                if let Some(mac) = &mut app.macro_lib.current_macro {
                    if let Some(Instruction::Token(InputToken::Key(_, direction))) =
                        mac.code.get(index).cloned()
                    {
                        let captured_key = map_iced_physical_key_to_macro_key(physical_key)
                            .or_else(|| map_iced_key_to_macro_key(key.as_ref()));

                        if let Some(captured_key) = captured_key {
                            mac.code[index] =
                                Instruction::Token(InputToken::Key(captured_key, direction));
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
                        app.editor_ui.invalid_field_buffers.clear();
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
                            app.editor_ui.invalid_field_buffers.clear();
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
                        tokio::time::sleep(std::time::Duration::from_secs(
                            CLEAR_CONFIRM_TIMEOUT_SECS,
                        ))
                        .await;
                        generation
                    },
                    |generation| ClearInstructionsTimeout(generation).into(),
                );
            } else {
                push_undo(app);
                if let Some(mac) = &mut app.macro_lib.current_macro {
                    mac.code.clear();
                    app.editor_ui.invalid_field_buffers.clear();
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
            app.editor_ui.invalid_field_buffers.clear();
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
                app.editor_ui.invalid_field_buffers.clear();
            }
        }
        ToggleLoopMode(enabled) => {
            app.execution.loop_mode_enabled = enabled;
            if let Err(err) =
                config::save_config_value(&app.config, "loop_mode_enabled", enabled)
            {
                warn!("Failed to save loop mode setting: {}", err);
            }
        }
        SetIpcPortText(text) => {
            match text.trim().parse::<u16>() {
                Ok(port) => {
                    app.editor_ui.ipc_port_invalid = false;
                    if let Err(err) = config::save_config_value(&app.config, "ipc_port", port) {
                        warn!("Failed to save IPC port setting: {}", err);
                    }
                }
                Err(_) => {
                    app.editor_ui.ipc_port_invalid = true;
                }
            }
            app.editor_ui.ipc_port_text = text;
        }
        StartIpcServer => {
            if app.execution.ipc_server.is_none() {
                if let Ok(port) = app.editor_ui.ipc_port_text.trim().parse::<u16>() {
                    let (tx, rx) = tokio::sync::watch::channel(false);
                    app.execution.ipc_server = Some(tokio::spawn(crate::ipc::run_server(port, rx)));
                    app.execution.ipc_shutdown_tx = Some(tx);
                    app.execution.ipc_active_port = Some(port);
                }
            }
        }
        StopIpcServer => {
            // Tell the accept loop and every connection it has spawned (e.g.
            // a persistent client like the Geode mod) to close. Aborting just
            // the accept loop's JoinHandle would only stop new connections —
            // already-accepted ones run as independent spawned tasks and
            // would keep being serviced forever.
            if let Some(tx) = app.execution.ipc_shutdown_tx.take() {
                let _ = tx.send(true);
            }
            if let Some(handle) = app.execution.ipc_server.take() {
                handle.abort();
            }
            app.execution.ipc_active_port = None;
        }
        SetIpcAutoStart(enabled) => {
            app.execution.ipc_auto_start = enabled;
            if let Err(err) = config::save_config_value(&app.config, "ipc_enabled", enabled) {
                warn!("Failed to save IPC auto-start setting: {}", err);
            }
        }
        #[cfg(windows)]
        CheckForUpdates => {
            app.editor_ui.update_check_state = UpdateCheckState::Checking;
            return Task::perform(
                async move {
                    let current_version = env!("CARGO_PKG_VERSION").to_string();
                    tokio::task::spawn_blocking(move || {
                        crate::updater::check_for_update(&current_version)
                    })
                    .await
                    .unwrap_or_else(|err| Err(format!("Task join error: {err}")))
                },
                |result| {
                    UpdateCheckResult(result.map(|opt| opt.map(|info| info.version))).into()
                },
            );
        }
        #[cfg(not(windows))]
        CheckForUpdates => {}
        UpdateCheckResult(result) => {
            app.editor_ui.update_check_state = match result {
                Ok(Some(version)) => UpdateCheckState::UpdateAvailable(version),
                Ok(None) => UpdateCheckState::UpToDate,
                Err(err) => UpdateCheckState::Error(err),
            };
        }
        #[cfg(windows)]
        ApplyUpdate => {
            app.editor_ui.update_check_state = UpdateCheckState::Applying;
            return Task::perform(
                async move {
                    let current_version = env!("CARGO_PKG_VERSION").to_string();
                    tokio::task::spawn_blocking(move || {
                        crate::updater::apply_update(&current_version)
                    })
                    .await
                    .unwrap_or_else(|err| Err(format!("Task join error: {err}")))
                },
                |result| UpdateApplyResult(result).into(),
            );
        }
        #[cfg(not(windows))]
        ApplyUpdate => {}
        UpdateApplyResult(result) => {
            match result {
                #[cfg(windows)]
                Ok(exe_path) => {
                    if let Err(err) = crate::updater::relaunch(&exe_path) {
                        app.editor_ui.update_check_state = UpdateCheckState::Error(err);
                    } else {
                        std::process::exit(0);
                    }
                }
                #[cfg(not(windows))]
                Ok(_) => {}
                Err(err) => {
                    app.editor_ui.update_check_state = UpdateCheckState::Error(err);
                }
            }
        }
        StartRecording => {
            if app.macro_lib.current_macro.is_none() {
                return Task::none();
            }
            app.editor_ui.recording_countdown_generation =
                app.editor_ui.recording_countdown_generation.wrapping_add(1);
            let countdown_gen = app.editor_ui.recording_countdown_generation;
            app.editor_ui.recording_phase = RecordingPhase::Countdown(5);
            return Task::perform(
                async move {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    countdown_gen
                },
                |g| RecordingCountdown(g).into(),
            );
        }
        RecordingCountdown(countdown_gen) => {
            if countdown_gen != app.editor_ui.recording_countdown_generation {
                return Task::none();
            }
            match app.editor_ui.recording_phase {
                RecordingPhase::Countdown(n) if n > 1 => {
                    app.editor_ui.recording_phase = RecordingPhase::Countdown(n - 1);
                    return Task::perform(
                        async move {
                            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                            countdown_gen
                        },
                        |g| RecordingCountdown(g).into(),
                    );
                }
                RecordingPhase::Countdown(_) => {
                    app.editor_ui.recording_phase = RecordingPhase::Active;
                    recording::reset_timing();
                    recording::RECORDING_ACTIVE.store(true, Ordering::Relaxed);
                }
                _ => {}
            }
        }
        ToggleRecordMouseRelative(relative) => {
            app.editor_ui.record_mouse_relative = relative; // TODO
            recording::RECORD_MOUSE_RELATIVE.store(relative, Ordering::Relaxed);
        }
        StopRecording => {
            app.editor_ui.recording_phase = RecordingPhase::Idle;
            let instructions: Vec<Instruction> = recording::get_recording_queue()
                .lock()
                .unwrap()
                .drain(..)
                .collect();
            if !instructions.is_empty() {
                push_undo(app);
                if let Some(mac) = &mut app.macro_lib.current_macro {
                    mac.code.extend(instructions);
                    auto_save_current_macro(app);
                }
                return scroll_editor_to_end();
            }
        }
        // Settings page
        OpenSettings => {
            app.editor_ui.page = Page::Settings;
            let bindings = config::load_hotkey_bindings(&app.config);
            app.editor_ui.hotkey_bindings = bindings;
            app.editor_ui.combo_capture = None;
            app.editor_ui.pending_macro_hotkey = None;
        }
        CloseSettings => {
            app.editor_ui.page = Page::Main;
            app.editor_ui.combo_capture = None;
            app.editor_ui.pending_macro_hotkey = None;
        }
        StartComboCapture(action) => {
            app.editor_ui.combo_capture = Some(ComboCapture::Named(action));
        }
        StartPendingComboCapture => {
            app.editor_ui.combo_capture = Some(ComboCapture::Pending);
        }
        SaveHotkeyBindings => {
            let bindings = app.editor_ui.hotkey_bindings.clone();
            if let Err(err) =
                config::save_config_value(&app.config, config::GLOBAL_HOTKEYS_KEY, &bindings)
            {
                warn!("Failed to save hotkey bindings: {}", err);
            }
            recording::update_hotkey_table(bindings);
        }
        SetPendingMacroIdx(idx) => {
            let entry = app
                .editor_ui
                .pending_macro_hotkey
                .get_or_insert((None, None));
            entry.0 = idx;
        }
        AddMacroHotkey => {
            if let Some((Some(idx), Some(combo))) =
                app.editor_ui.pending_macro_hotkey.take()
            {
                let macs = config::get_macros_from_config(&app.config);
                if let Some(mac) = macs.get(idx) {
                    let binding = HotkeyBinding {
                        action: HotkeyAction::RunSpecificMacro(mac.id.clone()),
                        combo,
                    };
                    app.editor_ui.hotkey_bindings.push(binding);
                    return handle_update(app, SaveHotkeyBindings);
                }
            }
        }
        RemoveHotkeyBinding(index) => {
            if index < app.editor_ui.hotkey_bindings.len() {
                app.editor_ui.hotkey_bindings.remove(index);
                return handle_update(app, SaveHotkeyBindings);
            }
        }
        ClearNamedHotkey(action) => {
            app.editor_ui
                .hotkey_bindings
                .retain(|b| b.action != action);
            return handle_update(app, SaveHotkeyBindings);
        }
        ResetHotkeyToDefault(action) => {
            if let Some(default_combo) = config::default_combo_for_action(&action) {
                if let Some(existing) = app
                    .editor_ui
                    .hotkey_bindings
                    .iter_mut()
                    .find(|b| b.action == action)
                {
                    existing.combo = default_combo;
                } else {
                    app.editor_ui.hotkey_bindings.push(HotkeyBinding {
                        action,
                        combo: default_combo,
                    });
                }
                return handle_update(app, SaveHotkeyBindings);
            }
        }
        EditorScrolled(offset_y, viewport_h) => {
            app.editor_ui.scroll_offset_y = offset_y;
            app.editor_ui.scroll_viewport_height = viewport_h;
        }
        NoOp => {}
        ExecuteHotkeyAction(action) => {
            match &action {
                HotkeyAction::RunMacro | HotkeyAction::RunSpecificMacro(_) => {
                    if let Some(emulator) = app.execution.emulator.as_ref() {
                        hotkeys::spawn_hotkey_action(
                            action,
                            app.config.clone(),
                            Arc::clone(emulator),
                            Arc::clone(&app.execution.is_looping),
                        );
                    }
                }
                HotkeyAction::StopLoop => {
                    hotkeys::stop_loop(&app.execution.is_looping);
                }
                HotkeyAction::NextMacro => {
                    let macs = config::get_macros_from_config(&app.config);
                    if !macs.is_empty() {
                        let current_id = config::get_selected_macro_id(&app.config);
                        let current_idx = current_id
                            .and_then(|id| macs.iter().position(|m| m.id == id))
                            .unwrap_or(0);
                        let next = (current_idx + 1) % macs.len();
                        let config = app.config.clone();
                        app.macro_lib.update_macro(&config, Some(next));
                    }
                }
                HotkeyAction::PrevMacro => {
                    let macs = config::get_macros_from_config(&app.config);
                    if !macs.is_empty() {
                        let current_id = config::get_selected_macro_id(&app.config);
                        let current_idx = current_id
                            .and_then(|id| macs.iter().position(|m| m.id == id))
                            .unwrap_or(0);
                        let prev = if current_idx == 0 {
                            macs.len() - 1
                        } else {
                            current_idx - 1
                        };
                        let config = app.config.clone();
                        app.macro_lib.update_macro(&config, Some(prev));
                    }
                }
                HotkeyAction::ToggleLoop => {
                    let new_val = !app.execution.loop_mode_enabled;
                    return handle_update(app, ToggleLoopMode(new_val));
                }
                HotkeyAction::StartRecordingImmediate => {
                    if app.macro_lib.current_macro.is_some() {
                        app.editor_ui.recording_countdown_generation =
                            app.editor_ui.recording_countdown_generation.wrapping_add(1);
                        app.editor_ui.recording_phase = RecordingPhase::Active;
                        recording::reset_timing();
                        recording::RECORDING_ACTIVE.store(true, Ordering::Relaxed);
                    }
                }
            }
        }
    }
    Task::none()
}

fn handle_combo_capture_event(app: &mut App, event: keyboard::Event) {
    match event {
        keyboard::Event::KeyPressed {
            physical_key,
            modifiers,
            key,
            ..
        } => {
            // Escape cancels capture
            if matches!(key.as_ref(), keyboard::Key::Named(keyboard::key::Named::Escape)) {
                app.editor_ui.combo_capture = None;
                return;
            }

            // Skip modifier-only presses
            if is_modifier_code(&physical_key) {
                return;
            }

            // Resolve rdev key name
            let key_name = match iced_code_to_rdev_key_name(&physical_key) {
                Some(n) => n,
                None => return,
            };

            let combo = KeyCombo {
                modifiers: mods_to_u8(&modifiers),
                key: key_name,
            };

            match app.editor_ui.combo_capture.take() {
                Some(ComboCapture::Named(action)) => {
                    // Upsert binding for this named action
                    if let Some(existing) = app
                        .editor_ui
                        .hotkey_bindings
                        .iter_mut()
                        .find(|b| b.action == action)
                    {
                        existing.combo = combo;
                    } else {
                        app.editor_ui.hotkey_bindings.push(HotkeyBinding {
                            action,
                            combo,
                        });
                    }
                    let _ = handle_update(app, SaveHotkeyBindings);
                }
                Some(ComboCapture::Pending) => {
                    let entry = app
                        .editor_ui
                        .pending_macro_hotkey
                        .get_or_insert((None, None));
                    entry.1 = Some(combo);
                }
                None => {}
            }
        }
        _ => {}
    }
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
