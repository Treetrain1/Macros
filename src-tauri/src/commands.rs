use crate::config;
use crate::hotkey_types::{HotkeyAction, HotkeyBinding, KeyCombo};
use crate::macros::{loop_control, thread, Instruction, Macro, Strand};
use crate::input::types::InputToken;
use crate::recording;
use crate::state::{
    build_state_dto, dto_to_hotkey_action, dto_to_instruction, emit_state_updated,
    ComboCapture, FieldId, HotkeyActionDto, InstructionDto, Page, RecordingPhase,
    SharedState, StateDto, UpdateCheckState,
};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{Runtime, State};
use tracing::warn;

const CLEAR_CONFIRM_TIMEOUT_SECS: u64 = 3;
const REMOVE_CONFIRM_TIMEOUT_SECS: u64 = 3;
const UNDO_STACK_LIMIT: usize = 50;

fn push_undo(s: &mut crate::state::AppState) {
    if let Some(mac) = &s.current_macro {
        if s.undo_stack.len() >= UNDO_STACK_LIMIT {
            s.undo_stack.remove(0);
        }
        s.undo_stack.push(mac.strands.clone());
        s.redo_stack.clear();
    }
}

/// Picks a default spawn position for a newly created/detached strand,
/// offset from the farthest-right strand so new stacks don't pile up on top
/// of existing ones.
fn next_strand_position(mac: &Macro) -> (i32, i32) {
    let max_x = mac.strands.iter().map(|s| s.x).max().unwrap_or(0);
    (max_x + 260, 0)
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
pub(crate) fn select_macro<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>, index: usize) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(mac) = s.macros_list.get(index).cloned() {
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
pub(crate) fn new_macro<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>) -> Result<(), String> {
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
pub(crate) fn remove_macro<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if !s.confirm_remove_macro {
        s.confirm_remove_macro = true;
        s.remove_confirm_remaining_secs = REMOVE_CONFIRM_TIMEOUT_SECS as u8;
        s.remove_confirm_generation = s.remove_confirm_generation.wrapping_add(1);
        let timeout_gen = s.remove_confirm_generation;
        emit_state_updated(&app, &s);
        drop(s);

        let state_clone = Arc::clone(&*state);
        let app_clone = app.clone();
        tauri::async_runtime::spawn(async move {
            for remaining in (1..=REMOVE_CONFIRM_TIMEOUT_SECS).rev() {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                if let Ok(mut s) = state_clone.lock() {
                    if s.remove_confirm_generation != timeout_gen {
                        break;
                    }
                    s.remove_confirm_remaining_secs = remaining as u8 - 1;
                    if remaining == 1 {
                        s.confirm_remove_macro = false;
                    }
                    emit_state_updated(&app_clone, &s);
                }
            }
        });
    } else {
        if let Some(mac) = s.current_macro.take() {
            if let Err(e) = mac.remove() {
                warn!("Failed to remove macro: {e}");
            }
        }
        s.confirm_remove_macro = false;
        s.remove_confirm_remaining_secs = 0;
        s.macro_selected = None;
        s.invalid_field_buffers.clear();
        refresh_macro_list(&mut s);
        config::set_selected_macro_id(None);
        emit_state_updated(&app, &s);
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn set_title<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>, title: String) -> Result<(), String> {
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
pub(crate) fn save_macro<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>) -> Result<(), String> {
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
pub(crate) fn add_instruction<R: Runtime>(
    state: State<SharedState>,
    app: tauri::AppHandle<R>,
    strand_id: String,
    index: usize,
    instruction: InstructionDto,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let ins = dto_to_instruction(&instruction).ok_or("Unknown instruction type")?;
    if let Some(mac) = &s.current_macro {
        if let Some(strand) = mac.strand(&strand_id) {
            let idx = index.min(strand.instructions.len());
            check_when_ran_attachment(strand, idx, &ins)?;
        }
    }
    push_undo(&mut s);
    if let Some(mac) = &mut s.current_macro {
        if let Some(strand) = mac.strand_mut(&strand_id) {
            let idx = index.min(strand.instructions.len());
            strand.instructions.insert(idx, ins);
            s.invalid_field_buffers.clear();
            auto_save(&s);
        }
    }
    emit_state_updated(&app, &s);
    Ok(())
}

/// Enforces the one rule governing "When Ran" blocks: nothing may ever end
/// up attached underneath one. That means inserting at index 0 of a strand
/// that already starts with `WhenRan` is forbidden (it would push the
/// existing one down to index 1), and inserting a `WhenRan` anywhere but
/// index 0 is forbidden (it would itself land underneath whatever's above
/// it).
fn check_when_ran_attachment(strand: &Strand, index: usize, ins: &Instruction) -> Result<(), String> {
    if index == 0 && strand.starts_with_when_ran() {
        return Err("Can't attach a block above a When Ran block".to_string());
    }
    if matches!(ins, Instruction::WhenRan) && index != 0 {
        return Err("A When Ran block can only be the first block in a strand".to_string());
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn edit_instruction<R: Runtime>(
    state: State<SharedState>,
    app: tauri::AppHandle<R>,
    strand_id: String,
    index: usize,
    instruction: InstructionDto,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let ins = dto_to_instruction(&instruction).ok_or("Unknown instruction type")?;
    if let Some(mac) = &mut s.current_macro {
        if let Some(strand) = mac.strand_mut(&strand_id) {
            if index < strand.instructions.len() {
                strand.instructions[index] = ins;
                auto_save(&s);
            }
        }
    }
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn edit_instruction_field<R: Runtime>(
    state: State<SharedState>,
    app: tauri::AppHandle<R>,
    strand_id: String,
    index: usize,
    field_id: String,
    text: String,
) -> Result<(), String> {
    let field: FieldId = field_id.parse().map_err(|e: String| e)?;
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(mac) = &mut s.current_macro {
        if let Some(strand) = mac.strand_mut(&strand_id) {
            if let Some(current) = strand.instructions.get(index).cloned() {
                let parsed_ok = match (&current, field) {
                    (Instruction::Wait(_, randomness), FieldId::WaitDuration) => {
                        text.parse::<f64>().map(|v| strand.instructions[index] = Instruction::Wait(v, *randomness)).is_ok()
                    }
                    (Instruction::Wait(duration, _), FieldId::WaitRandomness) => {
                        text.parse::<f64>().map(|v| strand.instructions[index] = Instruction::Wait(*duration, v)).is_ok()
                    }
                    (Instruction::Token(InputToken::MoveMouse(_, y, coord)), FieldId::MoveMouseX) => {
                        text.parse::<i32>().map(|v| strand.instructions[index] = Instruction::Token(InputToken::MoveMouse(v, *y, coord.clone()))).is_ok()
                    }
                    (Instruction::Token(InputToken::MoveMouse(x, _, coord)), FieldId::MoveMouseY) => {
                        text.parse::<i32>().map(|v| strand.instructions[index] = Instruction::Token(InputToken::MoveMouse(*x, v, coord.clone()))).is_ok()
                    }
                    (Instruction::Token(InputToken::Scroll(_, axis)), FieldId::ScrollAmount) => {
                        text.parse::<i32>().map(|v| strand.instructions[index] = Instruction::Token(InputToken::Scroll(v, axis.clone()))).is_ok()
                    }
                    _ => false,
                };
                s.invalid_field_buffers.insert((strand_id.clone(), index, field), text);
                if parsed_ok {
                    auto_save(&s);
                }
            }
        }
    }
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn remove_instruction<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>, strand_id: String, index: usize) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let should_remove = s.current_macro.as_ref()
        .and_then(|mac| mac.strand(&strand_id))
        .is_some_and(|strand| !strand.instructions.is_empty() && index < strand.instructions.len());
    if should_remove {
        push_undo(&mut s);
        if let Some(mac) = &mut s.current_macro {
            if let Some(strand) = mac.strand_mut(&strand_id) {
                strand.instructions.remove(index);
            }
            s.invalid_field_buffers.clear();
            auto_save(&s);
        }
    }
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn reorder_instruction<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>, strand_id: String, index: usize, direction: i32) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(mac) = &s.current_macro {
        if let Some(strand) = mac.strand(&strand_id) {
            let len = strand.instructions.len();
            if len > 1 && index < len {
                let new_index = if direction < 0 {
                    if index > 0 { index - 1 } else { index }
                } else if index < len - 1 {
                    index + 1
                } else {
                    index
                };
                // Swapping either end into position 0 would move a When Ran
                // block out of (or something else into) the head slot.
                let touches_when_ran_slot = (index == 0 || new_index == 0) && strand.starts_with_when_ran();
                if new_index != index && !touches_when_ran_slot {
                    push_undo(&mut s);
                    if let Some(mac) = &mut s.current_macro {
                        if let Some(strand) = mac.strand_mut(&strand_id) {
                            strand.instructions.swap(index, new_index);
                        }
                        s.invalid_field_buffers.clear();
                        auto_save(&s);
                    }
                }
            }
        }
    }
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn clear_instructions<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if !s.confirm_clear_instructions {
        s.confirm_clear_instructions = true;
        s.clear_confirm_remaining_secs = CLEAR_CONFIRM_TIMEOUT_SECS as u8;
        s.clear_confirm_generation = s.clear_confirm_generation.wrapping_add(1);
        let timeout_gen = s.clear_confirm_generation;
        emit_state_updated(&app, &s);
        drop(s);

        let state_clone = Arc::clone(&*state);
        let app_clone = app.clone();
        tauri::async_runtime::spawn(async move {
            for remaining in (1..=CLEAR_CONFIRM_TIMEOUT_SECS).rev() {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                if let Ok(mut s) = state_clone.lock() {
                    if s.clear_confirm_generation != timeout_gen {
                        break;
                    }
                    s.clear_confirm_remaining_secs = remaining as u8 - 1;
                    if remaining == 1 {
                        s.confirm_clear_instructions = false;
                    }
                    emit_state_updated(&app_clone, &s);
                }
            }
        });
    } else {
        push_undo(&mut s);
        if let Some(mac) = &mut s.current_macro {
            // Clearing wipes every strand's instructions, including any
            // "When Ran" blocks — matching "start this macro over from
            // scratch".
            for strand in &mut mac.strands {
                strand.instructions.clear();
            }
            s.invalid_field_buffers.clear();
            auto_save(&s);
            s.confirm_clear_instructions = false;
            s.clear_confirm_remaining_secs = 0;
        }
        emit_state_updated(&app, &s);
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn undo<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(prev_strands) = s.undo_stack.pop() {
        let current = s.current_macro.as_ref().map(|m| m.strands.clone());
        if let Some(cur) = current {
            s.redo_stack.push(cur);
        }
        if let Some(mac) = &mut s.current_macro {
            mac.strands = prev_strands;
            mac.ensure_id();
        }
        s.invalid_field_buffers.clear();
        auto_save(&s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn redo<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(next_strands) = s.redo_stack.pop() {
        let current = s.current_macro.as_ref().map(|m| m.strands.clone());
        if let Some(cur) = current {
            s.undo_stack.push(cur);
        }
        if let Some(mac) = &mut s.current_macro {
            mac.strands = next_strands;
            mac.ensure_id();
        }
        s.invalid_field_buffers.clear();
        auto_save(&s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

// ─── Strands (canvas) ──────────────────────────────────────────────────────

/// Creates a new detached strand. `x`/`y` default to an auto-picked spot
/// next to the farthest-right strand (used by the plain "add strand"
/// button); an explicit position is passed when this is the result of
/// dropping a palette block onto empty canvas. An initial `instruction` can
/// be supplied so a palette-block drop is one atomic, one-undo-step call
/// instead of create-then-move-then-add.
#[tauri::command]
pub(crate) fn add_strand<R: Runtime>(
    state: State<SharedState>,
    app: tauri::AppHandle<R>,
    x: Option<i32>,
    y: Option<i32>,
    instruction: Option<InstructionDto>,
) -> Result<String, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let ins = match instruction {
        Some(dto) => vec![dto_to_instruction(&dto).ok_or("Unknown instruction type")?],
        None => vec![],
    };
    push_undo(&mut s);
    let mac = s.current_macro.as_mut().ok_or("No macro selected")?;
    let (default_x, default_y) = next_strand_position(mac);
    let new_id = uuid::Uuid::new_v4().simple().to_string();
    mac.strands.push(Strand {
        id: new_id.clone(),
        x: x.unwrap_or(default_x),
        y: y.unwrap_or(default_y),
        instructions: ins,
    });
    auto_save(&s);
    emit_state_updated(&app, &s);
    Ok(new_id)
}

#[tauri::command]
pub(crate) fn remove_strand<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>, strand_id: String) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    push_undo(&mut s);
    if let Some(mac) = &mut s.current_macro {
        mac.strands.retain(|strand| strand.id != strand_id);
        s.invalid_field_buffers.retain(|(sid, _, _), _| sid != &strand_id);
        auto_save(&s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

/// Repositions a strand on the canvas — used while dragging a stack that
/// ends up dropped on empty space rather than snapped onto another strand.
#[tauri::command]
pub(crate) fn move_strand<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>, strand_id: String, x: i32, y: i32) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(mac) = &mut s.current_macro {
        if let Some(strand) = mac.strand_mut(&strand_id) {
            strand.x = x;
            strand.y = y;
            auto_save(&s);
        }
    }
    emit_state_updated(&app, &s);
    Ok(())
}

/// Detaches the instructions at and after `index` in `strand_id` into a new
/// strand positioned at `(x, y)`, returning the new strand's id. This is how
/// the frontend "picks up" a block: grabbing a block always splits it (and
/// everything stacked below it) off first, then either drops it as a new
/// stray strand or re-merges it elsewhere via `merge_strand`.
#[tauri::command]
pub(crate) fn split_strand<R: Runtime>(
    state: State<SharedState>,
    app: tauri::AppHandle<R>,
    strand_id: String,
    index: usize,
    x: i32,
    y: i32,
) -> Result<String, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    push_undo(&mut s);
    let mac = s.current_macro.as_mut().ok_or("No macro selected")?;
    let strand = mac.strand_mut(&strand_id).ok_or("Unknown strand")?;
    if index >= strand.instructions.len() {
        return Err("Split index out of range".to_string());
    }
    let tail = strand.instructions.split_off(index);
    let new_id = uuid::Uuid::new_v4().simple().to_string();
    mac.strands.push(Strand { id: new_id.clone(), x, y, instructions: tail });
    s.invalid_field_buffers.clear();
    auto_save(&s);
    emit_state_updated(&app, &s);
    Ok(new_id)
}

/// Splices `dragged_id`'s instructions into `target_id` at `index` and
/// deletes the (now empty) dragged strand — this is how two stacks snap
/// together on the canvas. A strand headed by a "When Ran" block can only
/// ever be a merge target (and only past its own head), never the dragged
/// side — merging it into another strand would attach it underneath
/// something, which is never allowed.
#[tauri::command]
pub(crate) fn merge_strand<R: Runtime>(
    state: State<SharedState>,
    app: tauri::AppHandle<R>,
    dragged_id: String,
    target_id: String,
    index: usize,
) -> Result<(), String> {
    if dragged_id == target_id {
        return Err("Can't merge a strand into itself".to_string());
    }
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let mac_ref = s.current_macro.as_ref().ok_or("No macro selected")?;
    let dragged_ref = mac_ref.strand(&dragged_id).ok_or("Unknown dragged strand")?;
    if dragged_ref.starts_with_when_ran() {
        return Err("A When Ran strand can't be merged into another strand".to_string());
    }
    if let Some(target_ref) = mac_ref.strand(&target_id) {
        if index == 0 && target_ref.starts_with_when_ran() {
            return Err("Can't attach a strand above a When Ran block".to_string());
        }
    }
    push_undo(&mut s);
    let mac = s.current_macro.as_mut().ok_or("No macro selected")?;
    let dragged_pos = mac.strands.iter().position(|s| s.id == dragged_id).ok_or("Unknown dragged strand")?;
    let dragged = mac.strands.remove(dragged_pos);
    let target = match mac.strand_mut(&target_id) {
        Some(t) => t,
        None => {
            // Target vanished (e.g. concurrent edit) — put the dragged
            // strand back rather than silently dropping its instructions.
            mac.strands.push(dragged);
            return Err("Unknown target strand".to_string());
        }
    };
    let idx = index.min(target.instructions.len());
    target.instructions.splice(idx..idx, dragged.instructions);
    s.invalid_field_buffers.retain(|(sid, _, _), _| sid != &dragged_id);
    auto_save(&s);
    emit_state_updated(&app, &s);
    Ok(())
}

// ─── Key capture ───────────────────────────────────────────────────────────

#[tauri::command]
pub(crate) fn start_key_capture<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>, strand_id: String, index: usize) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.key_capture = Some((strand_id, index));
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn key_capture_event<R: Runtime>(
    state: State<SharedState>,
    app: tauri::AppHandle<R>,
    code: String,
    key: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let Some((strand_id, index)) = s.key_capture.clone() else { return Ok(()); };

    let captured_key = crate::key_mapping::web_code_to_macro_key(&code)
        .or_else(|| crate::key_mapping::web_key_to_macro_key(&key));

    if let Some(mk) = captured_key {
        if let Some(mac) = &mut s.current_macro {
            if let Some(strand) = mac.strand_mut(&strand_id) {
                if let Some(Instruction::Token(InputToken::Key(_, dir))) = strand.instructions.get(index).cloned() {
                    strand.instructions[index] = Instruction::Token(InputToken::Key(mk, dir));
                    auto_save(&s);
                }
            }
        }
    }
    s.key_capture = None;
    emit_state_updated(&app, &s);
    Ok(())
}

// ─── Execution ─────────────────────────────────────────────────────────────

#[tauri::command]
pub(crate) fn run_macro<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>) -> Result<(), String> {
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
            let mac_name = mac.name.clone();
            let single_run_task = mac.into_single_run_task(Arc::clone(&emulator), Arc::clone(&is_looping));
            let mut s = state.lock().map_err(|e| e.to_string())?;
            if let Err(e) = thread::spawn_macro_thread(
                &mut s.thread_pool,
                format!("run_{}", mac_name),
                single_run_task,
            ) {
                warn!("Failed to spawn run thread: {e}");
                let _ = loop_control::stop_loop(&is_looping);
            }
        }
    }

    let s = state.lock().map_err(|e| e.to_string())?;
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn toggle_loop_mode<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>, enabled: bool) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.loop_mode_enabled = enabled;
    config::update_settings(|settings| settings.loop_mode_enabled = Some(enabled));
    emit_state_updated(&app, &s);
    Ok(())
}

// ─── Recording ─────────────────────────────────────────────────────────────

#[tauri::command]
pub(crate) fn start_recording<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if s.current_macro.is_none() { return Ok(()); }
    s.recording_countdown_generation = s.recording_countdown_generation.wrapping_add(1);
    let countdown_gen = s.recording_countdown_generation;
    s.recording_phase = RecordingPhase::Countdown(3);
    emit_state_updated(&app, &s);
    drop(s);

    let state_clone = Arc::clone(&*state);
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        for n in (0u8..3).rev() {
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
pub(crate) fn stop_recording<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>) -> Result<(), String> {
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
            mac.recording_target_mut().instructions.extend(instructions);
            auto_save(s);
        }
    }
}

/// Called from the QueueSignal background task when the OS-level hook signals stop.
pub(crate) fn stop_recording_internal<R: Runtime>(state: &SharedState, app: &tauri::AppHandle<R>) {
    if let Ok(mut s) = state.lock() {
        stop_recording_impl(&mut s);
        emit_state_updated(app, &s);
    }
}

#[tauri::command]
pub(crate) fn toggle_record_mouse_relative<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>, relative: bool) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.record_mouse_relative = relative;
    recording::RECORD_MOUSE_RELATIVE.store(relative, Ordering::Relaxed);
    emit_state_updated(&app, &s);
    Ok(())
}

// ─── Navigation ────────────────────────────────────────────────────────────

#[tauri::command]
pub(crate) fn open_settings<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.page = Page::Settings;
    s.hotkey_bindings = config::load_hotkey_bindings();
    s.combo_capture = None;
    s.pending_macro_hotkey = None;
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn close_settings<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.page = Page::Main;
    s.combo_capture = None;
    s.pending_macro_hotkey = None;
    emit_state_updated(&app, &s);
    Ok(())
}

// ─── Hotkey bindings ───────────────────────────────────────────────────────

#[tauri::command]
pub(crate) fn start_combo_capture<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>, action: HotkeyActionDto) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.combo_capture = Some(ComboCapture::Named(dto_to_hotkey_action(&action)));
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn start_pending_combo_capture<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.combo_capture = Some(ComboCapture::Pending);
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn combo_capture_event<R: Runtime>(
    state: State<SharedState>,
    app: tauri::AppHandle<R>,
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
pub(crate) fn cancel_combo_capture<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.combo_capture = None;
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn set_pending_macro_idx<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>, index: Option<usize>) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let entry = s.pending_macro_hotkey.get_or_insert((None, None));
    entry.0 = index;
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn add_macro_hotkey<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>) -> Result<(), String> {
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
pub(crate) fn remove_hotkey_binding<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>, index: usize) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if index < s.hotkey_bindings.len() {
        s.hotkey_bindings.remove(index);
        save_hotkey_bindings_impl(&mut s);
    }
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn clear_named_hotkey<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>, action: HotkeyActionDto) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let action = dto_to_hotkey_action(&action);
    s.hotkey_bindings.retain(|b| b.action != action);
    save_hotkey_bindings_impl(&mut s);
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn reset_hotkey_to_default<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>, action: HotkeyActionDto) -> Result<(), String> {
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
pub(crate) fn set_ipc_port_text<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>, text: String) -> Result<(), String> {
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
pub(crate) async fn start_ipc_server<R: Runtime>(state: State<'_, SharedState>, app: tauri::AppHandle<R>) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if s.ipc_server.is_none() {
        if let Ok(port) = s.ipc_port_text.trim().parse::<u16>() {
            let (tx, rx) = tokio::sync::watch::channel(false);
            s.ipc_server = Some(tauri::async_runtime::spawn(crate::ipc::run_server(port, rx)));
            s.ipc_shutdown_tx = Some(tx);
            s.ipc_active_port = Some(port);
        }
    }
    emit_state_updated(&app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn stop_ipc_server<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>) -> Result<(), String> {
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
pub(crate) fn set_ipc_auto_start<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>, enabled: bool) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.ipc_auto_start = enabled;
    config::update_settings(|settings| settings.ipc_auto_start = Some(enabled));
    emit_state_updated(&app, &s);
    Ok(())
}

// ─── Updates (Windows) ─────────────────────────────────────────────────────

#[tauri::command]
pub(crate) fn check_for_updates<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>) -> Result<(), String> {
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

pub(crate) async fn check_for_updates_internal<R: Runtime>(state: &SharedState, app: &tauri::AppHandle<R>) {
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
pub(crate) fn apply_update<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>) -> Result<(), String> {
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

pub(crate) fn handle_hotkey_action<R: Runtime>(state: &SharedState, app: &tauri::AppHandle<R>, action: HotkeyAction) {
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
                let mac = match &action {
                    HotkeyAction::RunMacro => s.current_macro.clone(),
                    HotkeyAction::RunSpecificMacro(id) => {
                        let macs = s.macros_list.clone();
                        macs.iter().find(|m| &m.id == id).cloned()
                    }
                    _ => None,
                };
                drop(s);
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
