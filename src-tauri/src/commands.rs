use crate::config;
use crate::hotkey_types::{HotkeyAction, HotkeyBinding, KeyCombo};
use crate::macros::{loop_control, thread, Instruction, Macro};
use crate::input::types::InputToken;
use crate::recording;
use crate::state::{
    build_state_dto, dto_to_hotkey_action, dto_to_instruction, emit_state_updated,
    ComboCapture, FieldId, HotkeyActionDto, InstructionDto, Page, RecordingPhase,
    SharedState, StateDto, UpdateCheckState,
};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::State;
use tracing::warn;

const CLEAR_CONFIRM_TIMEOUT_SECS: u64 = 5;
const UNDO_STACK_LIMIT: usize = 50;

fn push_undo(s: &mut crate::state::AppState) {
    if let Some(mac) = &s.current_macro {
        if s.undo_stack.len() >= UNDO_STACK_LIMIT {
            s.undo_stack.remove(0);
        }
        s.undo_stack.push(mac.code.clone());
        s.redo_stack.clear();
    }
}

fn refresh_macro_list(s: &mut crate::state::AppState) {
    let macros = config::get_macros_from_config();
    s.macro_strs = macros.iter().map(|m| m.name.clone()).collect();
    // Keep current_macro in sync if it was modified
    if let Some(ref cur) = s.current_macro.clone() {
        if let Some(updated) = macros.iter().find(|m| m.id == cur.id) {
            s.current_macro = Some(updated.clone());
        }
    }
    // Update macro_selected index (name may have changed)
    if let Some(ref cur) = s.current_macro {
        s.macro_selected = macros.iter().position(|m| m.id == cur.id);
    }
    s.macros_list = macros;
}

fn auto_save(s: &crate::state::AppState) {
    if let Some(mac) = &s.current_macro {
        if let Err(e) = mac.save() {
            warn!("Failed to auto-save macro: {e}");
        } else {
            config::set_selected_macro_id(Some(&mac.id));
        }
    }
}

// ─── Read ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub(crate) fn get_state(state: State<SharedState>) -> Result<StateDto, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    Ok(build_state_dto(&s))
}

// ─── Macro library ─────────────────────────────────────────────────────────

#[tauri::command]
pub(crate) fn select_macro(state: State<SharedState>, app: tauri::AppHandle, index: usize) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let macros = config::get_macros_from_config();
    if let Some(mac) = macros.get(index) {
        s.macro_selected = Some(index);
        s.current_macro = Some(mac.clone());
        config::set_selected_macro_id(Some(&mac.id));
    } else {
        s.macro_selected = None;
        s.current_macro = None;
        config::set_selected_macro_id(None);
    }
    s.undo_stack.clear();
    s.redo_stack.clear();
    s.invalid_field_buffers.clear();
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn new_macro(state: State<SharedState>, app: tauri::AppHandle) -> Result<(), String> {
    let new_macro = Macro::new("New Macro".into(), "".into(), vec![]);
    let new_id = new_macro.id.clone();
    if let Err(e) = new_macro.add() {
        return Err(format!("Failed to create macro: {e}"));
    }
    let mut s = state.lock().map_err(|e| e.to_string())?;
    refresh_macro_list(&mut s);
    // Select the newly created macro
    if let Some((idx, mac)) = s.macros_list.iter().enumerate().find(|(_, m)| m.id == new_id).map(|(i, m)| (i, m.clone())) {
        s.macro_selected = Some(idx);
        s.current_macro = Some(mac);
        config::set_selected_macro_id(Some(&new_id));
    }
    s.invalid_field_buffers.clear();
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn remove_macro(state: State<SharedState>, app: tauri::AppHandle) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if !s.confirm_remove_macro {
        s.confirm_remove_macro = true;
        emit_state_updated(&app, &s);
        return Ok(());
    }
    if let Some(mac) = s.current_macro.take() {
        if let Err(e) = mac.remove() {
            warn!("Failed to remove macro: {e}");
        }
    }
    s.confirm_remove_macro = false;
    s.macro_selected = None;
    s.invalid_field_buffers.clear();
    refresh_macro_list(&mut s);
    config::set_selected_macro_id(None);
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn set_title(state: State<SharedState>, app: tauri::AppHandle, title: String) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(mac) = &mut s.current_macro {
        mac.name = title;
        auto_save(&s);
        // Rebuild list so dropdown reflects the new name
        let macros = config::get_macros_from_config();
        s.macro_strs = macros.iter().map(|m| m.name.clone()).collect();
        s.macros_list = macros;
    }
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn save_macro(state: State<SharedState>, app: tauri::AppHandle) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(mac) = &s.current_macro {
        if let Err(e) = mac.save() {
            return Err(format!("Failed to save macro: {e}"));
        }
    }
    refresh_macro_list(&mut s);
    emit_state_updated(&app, &s);
    Ok(())
}

// ─── Instructions ──────────────────────────────────────────────────────────

#[tauri::command]
pub(crate) fn add_instruction(
    state: State<SharedState>,
    app: tauri::AppHandle,
    index: usize,
    instruction: InstructionDto,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let ins = dto_to_instruction(&instruction).ok_or("Unknown instruction type")?;
    push_undo(&mut s);
    if let Some(mac) = &mut s.current_macro {
        let idx = index.min(mac.code.len());
        mac.code.insert(idx, ins);
        s.invalid_field_buffers.clear();
        auto_save(&s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn edit_instruction(
    state: State<SharedState>,
    app: tauri::AppHandle,
    index: usize,
    instruction: InstructionDto,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let ins = dto_to_instruction(&instruction).ok_or("Unknown instruction type")?;
    if let Some(mac) = &mut s.current_macro {
        if index < mac.code.len() {
            mac.code[index] = ins;
            auto_save(&s);
        }
    }
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn edit_instruction_field(
    state: State<SharedState>,
    app: tauri::AppHandle,
    index: usize,
    field_id: String,
    text: String,
) -> Result<(), String> {
    let field: FieldId = field_id.parse().map_err(|e: String| e)?;
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(mac) = &mut s.current_macro {
        if let Some(current) = mac.code.get(index).cloned() {
            let parsed_ok = match (&current, field) {
                (Instruction::Wait(_, randomness), FieldId::WaitDuration) => {
                    text.parse::<f64>().map(|v| mac.code[index] = Instruction::Wait(v, *randomness)).is_ok()
                }
                (Instruction::Wait(duration, _), FieldId::WaitRandomness) => {
                    text.parse::<f64>().map(|v| mac.code[index] = Instruction::Wait(*duration, v)).is_ok()
                }
                (Instruction::Token(InputToken::MoveMouse(_, y, coord)), FieldId::MoveMouseX) => {
                    text.parse::<i32>().map(|v| mac.code[index] = Instruction::Token(InputToken::MoveMouse(v, *y, coord.clone()))).is_ok()
                }
                (Instruction::Token(InputToken::MoveMouse(x, _, coord)), FieldId::MoveMouseY) => {
                    text.parse::<i32>().map(|v| mac.code[index] = Instruction::Token(InputToken::MoveMouse(*x, v, coord.clone()))).is_ok()
                }
                (Instruction::Token(InputToken::Scroll(_, axis)), FieldId::ScrollAmount) => {
                    text.parse::<i32>().map(|v| mac.code[index] = Instruction::Token(InputToken::Scroll(v, axis.clone()))).is_ok()
                }
                _ => false,
            };
            s.invalid_field_buffers.insert((index, field), text);
            if parsed_ok {
                auto_save(&s);
            }
        }
    }
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn remove_instruction(state: State<SharedState>, app: tauri::AppHandle, index: usize) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(mac) = &s.current_macro {
        if !mac.code.is_empty() && index < mac.code.len() {
            push_undo(&mut s);
            if let Some(mac) = &mut s.current_macro {
                mac.code.remove(index);
                s.invalid_field_buffers.clear();
                auto_save(&s);
            }
        }
    }
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn reorder_instruction(state: State<SharedState>, app: tauri::AppHandle, index: usize, direction: i32) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(mac) = &s.current_macro {
        let len = mac.code.len();
        if len > 1 && index < len {
            let new_index = if direction < 0 {
                if index > 0 { index - 1 } else { index }
            } else if index < len - 1 {
                index + 1
            } else {
                index
            };
            if new_index != index {
                push_undo(&mut s);
                if let Some(mac) = &mut s.current_macro {
                    mac.code.swap(index, new_index);
                    s.invalid_field_buffers.clear();
                    auto_save(&s);
                }
            }
        }
    }
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn clear_instructions(state: State<SharedState>, app: tauri::AppHandle) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if !s.confirm_clear_instructions {
        s.confirm_clear_instructions = true;
        s.clear_confirm_generation = s.clear_confirm_generation.wrapping_add(1);
        let timeout_gen = s.clear_confirm_generation;
        emit_state_updated(&app, &s);
        drop(s);

        let state_clone = Arc::clone(&*state);
        let app_clone = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(CLEAR_CONFIRM_TIMEOUT_SECS)).await;
            if let Ok(mut s) = state_clone.lock() {
                if s.clear_confirm_generation == timeout_gen {
                    s.confirm_clear_instructions = false;
                    emit_state_updated(&app_clone, &s);
                }
            }
        });
    } else {
        push_undo(&mut s);
        if let Some(mac) = &mut s.current_macro {
            mac.code.clear();
            s.invalid_field_buffers.clear();
            auto_save(&s);
            s.confirm_clear_instructions = false;
        }
        emit_state_updated(&app, &s);
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn undo(state: State<SharedState>, app: tauri::AppHandle) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(prev_code) = s.undo_stack.pop() {
        let current = s.current_macro.as_ref().map(|m| m.code.clone());
        if let Some(cur) = current {
            s.redo_stack.push(cur);
        }
        if let Some(mac) = &mut s.current_macro {
            mac.code = prev_code;
        }
        s.invalid_field_buffers.clear();
        auto_save(&s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn redo(state: State<SharedState>, app: tauri::AppHandle) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(next_code) = s.redo_stack.pop() {
        let current = s.current_macro.as_ref().map(|m| m.code.clone());
        if let Some(cur) = current {
            s.undo_stack.push(cur);
        }
        if let Some(mac) = &mut s.current_macro {
            mac.code = next_code;
        }
        s.invalid_field_buffers.clear();
        auto_save(&s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

// ─── Key capture ───────────────────────────────────────────────────────────

#[tauri::command]
pub(crate) fn start_key_capture(state: State<SharedState>, app: tauri::AppHandle, index: usize) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.key_capture_index = Some(index);
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn key_capture_event(
    state: State<SharedState>,
    app: tauri::AppHandle,
    code: String,
    key: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let Some(index) = s.key_capture_index else { return Ok(()); };

    let captured_key = crate::key_mapping::web_code_to_macro_key(&code)
        .or_else(|| crate::key_mapping::web_key_to_macro_key(&key));

    if let Some(mk) = captured_key {
        if let Some(mac) = &s.current_macro {
            if let Some(Instruction::Token(InputToken::Key(_, dir))) = mac.code.get(index).cloned() {
                if let Some(mac) = &mut s.current_macro {
                    mac.code[index] = Instruction::Token(InputToken::Key(mk, dir));
                    auto_save(&s);
                }
            }
        }
    }
    s.key_capture_index = None;
    emit_state_updated(&app, &s);
    Ok(())
}

// ─── Execution ─────────────────────────────────────────────────────────────

#[tauri::command]
pub(crate) fn run_macro(state: State<SharedState>, app: tauri::AppHandle) -> Result<(), String> {
    let (mac, emulator, is_looping, loop_mode) = {
        let s = state.lock().map_err(|e| e.to_string())?;
        let mac = s.current_macro.clone();
        let emulator = s.emulator.as_ref().map(Arc::clone);
        let is_looping = Arc::clone(&s.is_looping);
        let loop_mode = s.loop_mode_enabled;
        (mac, emulator, is_looping, loop_mode)
    };

    if let (Some(mac), Some(emulator)) = (mac, emulator) {
        if loop_mode {
            if let Err(e) = loop_control::start_loop(&is_looping) {
                warn!("Failed to start loop: {e}");
                return Ok(());
            }
            let mac_name = mac.name.clone();
            let loop_task = mac.into_loop_task(Arc::clone(&emulator), Arc::clone(&is_looping));
            let mut s = state.lock().map_err(|e| e.to_string())?;
            if let Err(e) = thread::spawn_macro_thread(
                &mut s.thread_pool,
                format!("loop_{}", mac_name),
                loop_task,
            ) {
                warn!("Failed to spawn loop thread: {e}");
                let _ = loop_control::stop_loop(&is_looping);
            }
        } else {
            let _ = loop_control::set_loop_state(&is_looping, true);
            let stop_flag = Arc::clone(&is_looping);
            tokio::task::spawn_blocking(move || {
                #[cfg(windows)]
                crate::macros::backend::windows::prepare_for_macro_execution();
                mac.run(emulator, Some(Arc::clone(&stop_flag)));
                if let Ok(mut st) = stop_flag.lock() { *st = false; }
            });
        }
    }

    let s = state.lock().map_err(|e| e.to_string())?;
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn toggle_loop_mode(state: State<SharedState>, app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.loop_mode_enabled = enabled;
    config::update_settings(|settings| settings.loop_mode_enabled = Some(enabled));
    emit_state_updated(&app, &s);
    Ok(())
}

// ─── Recording ─────────────────────────────────────────────────────────────

#[tauri::command]
pub(crate) fn start_recording(state: State<SharedState>, app: tauri::AppHandle) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if s.current_macro.is_none() { return Ok(()); }
    s.recording_countdown_generation = s.recording_countdown_generation.wrapping_add(1);
    let countdown_gen = s.recording_countdown_generation;
    s.recording_phase = RecordingPhase::Countdown(5);
    emit_state_updated(&app, &s);
    drop(s);

    let state_clone = Arc::clone(&*state);
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        for n in (0u8..5).rev() {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            let mut s = match state_clone.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            if s.recording_countdown_generation != countdown_gen { return; }
            if n == 0 {
                s.recording_phase = RecordingPhase::Active;
                recording::reset_timing();
                recording::RECORDING_ACTIVE.store(true, Ordering::Relaxed);
                emit_state_updated(&app_clone, &s);
                return;
            }
            s.recording_phase = RecordingPhase::Countdown(n);
            emit_state_updated(&app_clone, &s);
        }
    });
    Ok(())
}

#[tauri::command]
pub(crate) fn stop_recording(state: State<SharedState>, app: tauri::AppHandle) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    stop_recording_impl(&mut s);
    emit_state_updated(&app, &s);
    Ok(())
}

fn stop_recording_impl(s: &mut crate::state::AppState) {
    recording::RECORDING_ACTIVE.store(false, Ordering::Relaxed);
    s.recording_phase = RecordingPhase::Idle;
    // Cancel any in-progress countdown
    s.recording_countdown_generation = s.recording_countdown_generation.wrapping_add(1);

    let instructions: Vec<Instruction> = recording::get_recording_queue()
        .lock()
        .unwrap()
        .drain(..)
        .collect();
    if !instructions.is_empty() {
        push_undo(s);
        if let Some(mac) = &mut s.current_macro {
            mac.code.extend(instructions);
            auto_save(s);
        }
    }
}

/// Called from the QueueSignal background task when the OS-level hook signals stop.
pub(crate) fn stop_recording_internal(state: &SharedState, app: &tauri::AppHandle) {
    if let Ok(mut s) = state.lock() {
        stop_recording_impl(&mut s);
        emit_state_updated(app, &s);
    }
}

#[tauri::command]
pub(crate) fn toggle_record_mouse_relative(state: State<SharedState>, app: tauri::AppHandle, relative: bool) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.record_mouse_relative = relative;
    recording::RECORD_MOUSE_RELATIVE.store(relative, Ordering::Relaxed);
    emit_state_updated(&app, &s);
    Ok(())
}

// ─── Navigation ────────────────────────────────────────────────────────────

#[tauri::command]
pub(crate) fn open_settings(state: State<SharedState>, app: tauri::AppHandle) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.page = Page::Settings;
    s.hotkey_bindings = config::load_hotkey_bindings();
    s.combo_capture = None;
    s.pending_macro_hotkey = None;
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn close_settings(state: State<SharedState>, app: tauri::AppHandle) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.page = Page::Main;
    s.combo_capture = None;
    s.pending_macro_hotkey = None;
    emit_state_updated(&app, &s);
    Ok(())
}

// ─── Hotkey bindings ───────────────────────────────────────────────────────

#[tauri::command]
pub(crate) fn start_combo_capture(state: State<SharedState>, app: tauri::AppHandle, action: HotkeyActionDto) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.combo_capture = Some(ComboCapture::Named(dto_to_hotkey_action(&action)));
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn start_pending_combo_capture(state: State<SharedState>, app: tauri::AppHandle) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.combo_capture = Some(ComboCapture::Pending);
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn combo_capture_event(
    state: State<SharedState>,
    app: tauri::AppHandle,
    code: String,
    modifiers: u8,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let key_name = match crate::key_mapping::web_code_to_rdev_name(&code) {
        Some(n) => n,
        None => return Ok(()),
    };
    let combo = KeyCombo { modifiers, key: key_name };

    match s.combo_capture.take() {
        Some(ComboCapture::Named(action)) => {
            if let Some(existing) = s.hotkey_bindings.iter_mut().find(|b| b.action == action) {
                existing.combo = combo;
            } else {
                s.hotkey_bindings.push(HotkeyBinding { action, combo });
            }
            save_hotkey_bindings_impl(&mut s);
        }
        Some(ComboCapture::Pending) => {
            let entry = s.pending_macro_hotkey.get_or_insert((None, None));
            entry.1 = Some(combo);
        }
        None => {}
    }
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn cancel_combo_capture(state: State<SharedState>, app: tauri::AppHandle) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.combo_capture = None;
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn set_pending_macro_idx(state: State<SharedState>, app: tauri::AppHandle, index: Option<usize>) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let entry = s.pending_macro_hotkey.get_or_insert((None, None));
    entry.0 = index;
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn add_macro_hotkey(state: State<SharedState>, app: tauri::AppHandle) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some((Some(idx), Some(combo))) = s.pending_macro_hotkey.take() {
        let macros = config::get_macros_from_config();
        if let Some(mac) = macros.get(idx) {
            let binding = HotkeyBinding {
                action: HotkeyAction::RunSpecificMacro(mac.id.clone()),
                combo,
            };
            s.hotkey_bindings.push(binding);
            save_hotkey_bindings_impl(&mut s);
        }
    }
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn remove_hotkey_binding(state: State<SharedState>, app: tauri::AppHandle, index: usize) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if index < s.hotkey_bindings.len() {
        s.hotkey_bindings.remove(index);
        save_hotkey_bindings_impl(&mut s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn clear_named_hotkey(state: State<SharedState>, app: tauri::AppHandle, action: HotkeyActionDto) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let action = dto_to_hotkey_action(&action);
    s.hotkey_bindings.retain(|b| b.action != action);
    save_hotkey_bindings_impl(&mut s);
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn reset_hotkey_to_default(state: State<SharedState>, app: tauri::AppHandle, action: HotkeyActionDto) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let action = dto_to_hotkey_action(&action);
    if let Some(default_combo) = config::default_combo_for_action(&action) {
        if let Some(existing) = s.hotkey_bindings.iter_mut().find(|b| b.action == action) {
            existing.combo = default_combo;
        } else {
            s.hotkey_bindings.push(HotkeyBinding { action, combo: default_combo });
        }
        save_hotkey_bindings_impl(&mut s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

fn save_hotkey_bindings_impl(s: &mut crate::state::AppState) {
    config::save_hotkey_bindings(&s.hotkey_bindings);
    recording::update_hotkey_table(s.hotkey_bindings.clone());
}

// ─── IPC server ────────────────────────────────────────────────────────────

#[tauri::command]
pub(crate) fn set_ipc_port_text(state: State<SharedState>, app: tauri::AppHandle, text: String) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    match text.trim().parse::<u16>() {
        Ok(port) => {
            s.ipc_port_invalid = false;
            config::update_settings(|settings| settings.ipc_port = Some(port));
        }
        Err(_) => {
            s.ipc_port_invalid = true;
        }
    }
    s.ipc_port_text = text;
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn start_ipc_server(state: State<SharedState>, app: tauri::AppHandle) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if s.ipc_server.is_none() {
        if let Ok(port) = s.ipc_port_text.trim().parse::<u16>() {
            let (tx, rx) = tokio::sync::watch::channel(false);
            s.ipc_server = Some(tokio::spawn(crate::ipc::run_server(port, rx)));
            s.ipc_shutdown_tx = Some(tx);
            s.ipc_active_port = Some(port);
        }
    }
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn stop_ipc_server(state: State<SharedState>, app: tauri::AppHandle) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(tx) = s.ipc_shutdown_tx.take() {
        let _ = tx.send(true);
    }
    if let Some(handle) = s.ipc_server.take() {
        handle.abort();
    }
    s.ipc_active_port = None;
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn set_ipc_auto_start(state: State<SharedState>, app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.ipc_auto_start = enabled;
    config::update_settings(|settings| settings.ipc_auto_start = Some(enabled));
    emit_state_updated(&app, &s);
    Ok(())
}

// ─── Updates (Windows) ─────────────────────────────────────────────────────

#[tauri::command]
pub(crate) fn check_for_updates(state: State<SharedState>, app: tauri::AppHandle) -> Result<(), String> {
    {
        let mut s = state.lock().map_err(|e| e.to_string())?;
        s.update_check_state = UpdateCheckState::Checking;
        emit_state_updated(&app, &s);
    }
    let state_clone = Arc::clone(&*state);
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        check_for_updates_internal(&state_clone, &app_clone).await;
    });
    Ok(())
}

pub(crate) async fn check_for_updates_internal(state: &SharedState, app: &tauri::AppHandle) {
    #[cfg(windows)]
    {
        let version = env!("CARGO_PKG_VERSION").to_string();
        let result = tokio::task::spawn_blocking(move || crate::updater::check_for_update(&version))
            .await
            .unwrap_or_else(|e| Err(e.to_string()));
        if let Ok(mut s) = state.lock() {
            s.update_check_state = match result {
                Ok(Some(info)) => UpdateCheckState::UpdateAvailable(info.version),
                Ok(None) => UpdateCheckState::UpToDate,
                Err(e) => UpdateCheckState::Error(e),
            };
            emit_state_updated(app, &s);
        }
    }
    #[cfg(not(windows))]
    let _ = (state, app);
}

#[tauri::command]
pub(crate) fn apply_update(state: State<SharedState>, app: tauri::AppHandle) -> Result<(), String> {
    {
        let mut s = state.lock().map_err(|e| e.to_string())?;
        s.update_check_state = UpdateCheckState::Applying;
        emit_state_updated(&app, &s);
    }
    #[cfg(windows)]
    {
        let state_clone = Arc::clone(&*state);
        let app_clone = app.clone();
        tauri::async_runtime::spawn(async move {
            let version = env!("CARGO_PKG_VERSION").to_string();
            let result = tokio::task::spawn_blocking(move || crate::updater::apply_update(&version))
                .await
                .unwrap_or_else(|e| Err(e.to_string()));
            match result {
                Ok(_) => std::process::exit(0),
                Err(e) => {
                    if let Ok(mut s) = state_clone.lock() {
                        s.update_check_state = UpdateCheckState::Error(e);
                        emit_state_updated(&app_clone, &s);
                    }
                }
            }
        });
    }
    Ok(())
}

// ─── Hotkey action handler (called from QueueSignal task) ──────────────────

pub(crate) fn handle_hotkey_action(state: &SharedState, app: &tauri::AppHandle, action: HotkeyAction) {
    match &action {
        HotkeyAction::RunMacro | HotkeyAction::RunSpecificMacro(_) => {
            let s = match state.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            if let Some(emulator) = s.emulator.as_ref() {
                let emulator = Arc::clone(emulator);
                let is_looping = Arc::clone(&s.is_looping);
                let loop_mode = s.loop_mode_enabled;
                let macros_list = s.macros_list.clone();
                let selected_id = config::get_selected_macro_id();
                drop(s);
                let mac = match &action {
                    HotkeyAction::RunMacro => {
                        selected_id.and_then(|id| macros_list.iter().find(|m| m.id == id).cloned())
                    }
                    HotkeyAction::RunSpecificMacro(id) => {
                        macros_list.iter().find(|m| &m.id == id).cloned()
                    }
                    _ => None,
                };
                if let Some(mac) = mac {
                    run_macro_task(mac, emulator, is_looping, loop_mode);
                }
            }
        }
        HotkeyAction::StopLoop => {
            if let Ok(s) = state.lock() {
                if let Ok(mut lk) = s.is_looping.lock() {
                    *lk = false;
                }
            }
        }
        HotkeyAction::NextMacro => {
            let macros = config::get_macros_from_config();
            if !macros.is_empty() {
                if let Ok(mut s) = state.lock() {
                    let current_id = config::get_selected_macro_id();
                    let current_idx = current_id
                        .and_then(|id| macros.iter().position(|m| m.id == id))
                        .unwrap_or(0);
                    let next = (current_idx + 1) % macros.len();
                    s.macro_selected = Some(next);
                    s.current_macro = macros.get(next).cloned();
                    if let Some(mac) = &s.current_macro {
                        config::set_selected_macro_id(Some(&mac.id));
                    }
                    emit_state_updated(app, &s);
                }
            }
        }
        HotkeyAction::PrevMacro => {
            let macros = config::get_macros_from_config();
            if !macros.is_empty() {
                if let Ok(mut s) = state.lock() {
                    let current_id = config::get_selected_macro_id();
                    let current_idx = current_id
                        .and_then(|id| macros.iter().position(|m| m.id == id))
                        .unwrap_or(0);
                    let prev = if current_idx == 0 { macros.len() - 1 } else { current_idx - 1 };
                    s.macro_selected = Some(prev);
                    s.current_macro = macros.get(prev).cloned();
                    if let Some(mac) = &s.current_macro {
                        config::set_selected_macro_id(Some(&mac.id));
                    }
                    emit_state_updated(app, &s);
                }
            }
        }
        HotkeyAction::ToggleLoop => {
            if let Ok(mut s) = state.lock() {
                s.loop_mode_enabled = !s.loop_mode_enabled;
                let enabled = s.loop_mode_enabled;
                config::update_settings(|settings| settings.loop_mode_enabled = Some(enabled));
                emit_state_updated(app, &s);
            }
        }
        HotkeyAction::StartRecordingImmediate => {
            if let Ok(mut s) = state.lock() {
                if s.current_macro.is_some() {
                    s.recording_countdown_generation = s.recording_countdown_generation.wrapping_add(1);
                    s.recording_phase = RecordingPhase::Active;
                    recording::reset_timing();
                    recording::RECORDING_ACTIVE.store(true, Ordering::Relaxed);
                    emit_state_updated(app, &s);
                }
            }
        }
    }
}

fn run_macro_task(
    mac: Macro,
    emulator: Arc<std::sync::Mutex<dyn crate::macros::backend::InputBackend>>,
    is_looping: Arc<std::sync::Mutex<bool>>,
    loop_mode: bool,
) {
    if loop_mode {
        if let Ok(mut st) = is_looping.lock() { *st = true; }
        let loop_flag = Arc::clone(&is_looping);
        tokio::task::spawn_blocking(move || {
            #[cfg(windows)]
            crate::macros::backend::windows::prepare_for_macro_execution();
            loop {
                if let Ok(should_continue) = loop_flag.lock() {
                    if !*should_continue { break; }
                } else {
                    break;
                }
                mac.clone().run(Arc::clone(&emulator), Some(Arc::clone(&loop_flag)));
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        });
    } else {
        if let Ok(mut st) = is_looping.lock() { *st = true; }
        let stop_flag = Arc::clone(&is_looping);
        tokio::task::spawn_blocking(move || {
            #[cfg(windows)]
            crate::macros::backend::windows::prepare_for_macro_execution();
            mac.run(emulator, Some(Arc::clone(&stop_flag)));
            if let Ok(mut st) = stop_flag.lock() { *st = false; }
        });
    }
}
