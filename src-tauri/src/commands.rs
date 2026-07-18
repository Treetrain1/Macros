use crate::config;
use crate::hotkey_types::{HotkeyAction, HotkeyBinding, KeyCombo};
use crate::macros::{loop_control, thread, FloatingValue, Instruction, Macro, Strand, SPEED_MULTIPLIER_RANGE};
use crate::input::types::InputToken;
use crate::input::value::{Evaluated, Op, Value};
use crate::recording;
use crate::state::{
    build_state_dto, dto_to_hotkey_action, dto_to_instruction, dto_to_value, emit_state_updated,
    value_to_dto, ComboCapture, FieldId, HotkeyActionDto, InstructionDto, MacroSnapshot, Page,
    RecordingPhase, SharedState, StateDto, UpdateCheckState, ValueDto, ValueLocation, ValueLocationDto,
};
use std::collections::HashMap;
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
        s.undo_stack.push(MacroSnapshot { strands: mac.strands.clone(), floating_values: mac.floating_values.clone() });
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
pub(crate) fn set_macro_speed_multiplier<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>, multiplier: f64) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(mac) = &mut s.current_macro {
        mac.speed_multiplier = multiplier.clamp(*SPEED_MULTIPLIER_RANGE.start(), *SPEED_MULTIPLIER_RANGE.end());
        auto_save(&s);
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

/// Strips characters illegal in filenames on common platforms, so a macro's
/// freeform name can double as a sane default export filename.
fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() { "macro".to_string() } else { trimmed.to_string() }
}

/// Exports a single macro to a user-chosen `.macro` file (same JSON shape as
/// the app's own on-disk storage, just under a different extension so it
/// reads as a portable macro file rather than an app-internal one).
#[tauri::command]
pub(crate) async fn export_macro(macro_id: String) -> Result<(), String> {
    let macros = config::get_macros_from_config();
    let mac = macros.into_iter().find(|m| m.id == macro_id).ok_or("Macro not found")?;

    let file = rfd::AsyncFileDialog::new()
        .set_title("Export Macro")
        .set_file_name(&format!("{}.macro", sanitize_filename(&mac.name)))
        .add_filter("Macro", &["macro"])
        .save_file()
        .await;

    let Some(file) = file else { return Ok(()); };
    config::write_macro_file(file.path(), &mac)
}

/// Imports a `.macro` file as a brand-new macro (fresh id, so importing never
/// collides with or overwrites an existing macro — even a re-imported copy of
/// one already in the library lands as a separate entry).
#[tauri::command]
pub(crate) async fn import_macro<R: Runtime>(state: State<'_, SharedState>, app: tauri::AppHandle<R>) -> Result<(), String> {
    let file = rfd::AsyncFileDialog::new()
        .set_title("Import Macro")
        .add_filter("Macro", &["macro"])
        .pick_file()
        .await;

    let Some(file) = file else { return Ok(()); };
    let mut mac = config::read_macro_file(file.path())?;
    mac.id = uuid::Uuid::new_v4().simple().to_string();
    let new_id = mac.id.clone();
    mac.add()?;

    let mut s = state.lock().map_err(|e| e.to_string())?;
    refresh_macro_list(&mut s);
    if let Some((idx, found)) = s.macros_list.iter().enumerate().find(|(_, m)| m.id == new_id).map(|(i, m)| (i, m.clone())) {
        s.macro_selected = Some(idx);
        s.current_macro = Some(found);
        config::set_selected_macro_id(Some(&new_id));
    }
    s.invalid_field_buffers.clear();
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
    if ins.is_header() && index != 0 {
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

/// Locates the `Value` tree a `FieldId` names on a given instruction — the
/// root that a `ValueLocation::Field`'s `path` then walks into via
/// [`Value::get_mut`].
fn value_slot_mut(ins: &mut Instruction, field: FieldId) -> Option<&mut Value> {
    match (ins, field) {
        (Instruction::Wait(d, _), FieldId::WaitDuration) => Some(d),
        (Instruction::Wait(_, r), FieldId::WaitRandomness) => Some(r),
        (Instruction::Token(InputToken::MoveMouse(x, _, _)), FieldId::MoveMouseX) => Some(x),
        (Instruction::Token(InputToken::MoveMouse(_, y, _)), FieldId::MoveMouseY) => Some(y),
        (Instruction::Token(InputToken::Scroll(a, _)), FieldId::ScrollAmount) => Some(a),
        _ => None,
    }
}

/// Resolves a `ValueLocation` (a field slot inside an instruction, or a
/// value block parked on canvas) down to the specific `Value` node it
/// addresses — the one function every value-editing command in this file
/// routes through.
fn resolve_location_mut<'a>(mac: &'a mut Macro, location: &ValueLocation) -> Option<&'a mut Value> {
    match location {
        ValueLocation::Field { strand_id, index, field_id, path } => {
            let strand = mac.strand_mut(strand_id)?;
            let ins = strand.instructions.get_mut(*index)?;
            value_slot_mut(ins, *field_id)?.get_mut(path)
        }
        ValueLocation::Floating { floating_id, path } => mac.floating_value_mut(floating_id)?.value.get_mut(path),
    }
}

/// `MoveMouseX/Y` and `ScrollAmount` were always `i32` fields; a `Number`
/// leaf under one of them keeps that integer-only constraint even now that
/// it's nested inside a `Value` tree. `WaitDuration`/`WaitRandomness`, and
/// every floating value block (not tied to any specific instruction field),
/// allow decimals.
fn location_requires_integer(location: &ValueLocation) -> bool {
    matches!(location, ValueLocation::Field { field_id, .. }
        if matches!(field_id, FieldId::MoveMouseX | FieldId::MoveMouseY | FieldId::ScrollAmount))
}

/// Drops any buffered invalid-text entries in the same tree as `location`
/// whose path is `location`'s path itself or a descendant of it — used
/// after a subtree gets replaced wholesale (kind change, take/put), since
/// any buffered text for a leaf that no longer exists there would otherwise
/// linger and be shown against the wrong node.
fn prune_value_buffers(buffers: &mut HashMap<ValueLocation, String>, location: &ValueLocation) {
    let path = location.path();
    buffers.retain(|loc, _| !(loc.same_root(location) && loc.path().starts_with(path)));
}

/// Applies `kind`'s default construction to `node`, in place — used by
/// `set_value_kind` for dropping a fresh block from the sidebar onto an
/// occupied slot (best-effort keeps the old leaf rather than discarding it,
/// see the match arms below).
fn apply_value_kind(node: &mut Value, kind: &str) -> Result<(), String> {
    match kind {
        "Number" => {
            // Best-effort: collapsing `(2)+(3)` gives `5` instead of
            // discarding the user's work.
            let n = node.eval().and_then(|e| e.as_number()).unwrap_or(0.0);
            *node = Value::number(n);
        }
        "Text" => {
            let text = match node.eval() {
                Ok(Evaluated::Text(s)) => s,
                Ok(Evaluated::Number(n)) => n.to_string(),
                Err(_) => String::new(),
            };
            *node = Value::Text { value: text };
        }
        "Add" | "Sub" | "Mul" | "Div" => {
            let op = match kind {
                "Sub" => Op::Sub,
                "Mul" => Op::Mul,
                "Div" => Op::Div,
                _ => Op::Add,
            };
            // Already an operator: just swap it, keeping both operands (flip
            // `+` to `×` without losing work). Otherwise promote the leaf
            // into the new lhs.
            let existing = std::mem::replace(node, Value::number(0.0));
            *node = match existing {
                Value::BinaryOp { lhs, rhs, .. } => Value::BinaryOp { op, lhs, rhs },
                other => Value::BinaryOp { op, lhs: Box::new(other), rhs: Box::new(Value::number(0.0)) },
            };
        }
        _ => return Err(format!("Unknown value kind: {kind}")),
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn edit_value_field<R: Runtime>(
    state: State<SharedState>,
    app: tauri::AppHandle<R>,
    location: ValueLocationDto,
    text: String,
) -> Result<(), String> {
    let loc = location.to_location()?;
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(mac) = &mut s.current_macro {
        if let Some(node) = resolve_location_mut(mac, &loc) {
            if matches!(node, Value::Text { .. }) {
                // Text leaves are always valid — no invalid-buffer
                // bookkeeping, unlike the numeric case below.
                *node = Value::Text { value: text };
                auto_save(&s);
            } else {
                let parsed = if location_requires_integer(&loc) {
                    text.parse::<i32>().map(|v| v as f64).map_err(|_| ())
                } else {
                    text.parse::<f64>().map_err(|_| ())
                };
                let parsed_ok = parsed.map(|v| *node = Value::number(v)).is_ok();
                s.invalid_field_buffers.insert(loc.clone(), text);
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
pub(crate) fn set_value_kind<R: Runtime>(
    state: State<SharedState>,
    app: tauri::AppHandle<R>,
    location: ValueLocationDto,
    kind: String,
) -> Result<(), String> {
    let loc = location.to_location()?;
    let mut s = state.lock().map_err(|e| e.to_string())?;
    push_undo(&mut s);
    let result = if let Some(mac) = &mut s.current_macro {
        match resolve_location_mut(mac, &loc) {
            Some(node) => apply_value_kind(node, &kind),
            None => Ok(()),
        }
    } else {
        Ok(())
    };
    prune_value_buffers(&mut s.invalid_field_buffers, &loc);
    auto_save(&s);
    emit_state_updated(&app, &s);
    result
}

/// Removes the value at `location` and returns it — a `Field` location is
/// left holding `Value::number(0.0)`; a `Floating` location at its own root
/// (`path == []`) is deleted entirely; a `Floating` location at a nested
/// path is left holding `Value::number(0.0)`, like `Field`. The first half
/// of "drag an existing block somewhere else" — the frontend follows up
/// with `put_value`/`create_floating_value` (or nothing, if the drop turned
/// out to be a no-op).
#[tauri::command]
pub(crate) fn take_value<R: Runtime>(
    state: State<SharedState>,
    app: tauri::AppHandle<R>,
    location: ValueLocationDto,
) -> Result<ValueDto, String> {
    let loc = location.to_location()?;
    let mut s = state.lock().map_err(|e| e.to_string())?;
    push_undo(&mut s);
    let taken = (|| {
        let mac = s.current_macro.as_mut()?;
        if let ValueLocation::Floating { floating_id, path } = &loc {
            if path.is_empty() {
                let idx = mac.floating_values.iter().position(|f| &f.id == floating_id)?;
                return Some(mac.floating_values.remove(idx).value);
            }
        }
        let node = resolve_location_mut(mac, &loc)?;
        Some(std::mem::replace(node, Value::number(0.0)))
    })();
    prune_value_buffers(&mut s.invalid_field_buffers, &loc);
    auto_save(&s);
    emit_state_updated(&app, &s);
    match taken {
        Some(v) => Ok(value_to_dto(&v)),
        None => Err("Nothing to take at that location".to_string()),
    }
}

/// Overwrites the node at `location` with `value` — the "put" half of
/// moving an existing block into a field/subfield slot.
#[tauri::command]
pub(crate) fn put_value<R: Runtime>(
    state: State<SharedState>,
    app: tauri::AppHandle<R>,
    location: ValueLocationDto,
    value: ValueDto,
) -> Result<(), String> {
    let loc = location.to_location()?;
    let mut s = state.lock().map_err(|e| e.to_string())?;
    push_undo(&mut s);
    if let Some(mac) = &mut s.current_macro {
        if let Some(node) = resolve_location_mut(mac, &loc) {
            *node = dto_to_value(&value);
        }
    }
    prune_value_buffers(&mut s.invalid_field_buffers, &loc);
    auto_save(&s);
    emit_state_updated(&app, &s);
    Ok(())
}

/// Creates a new value block parked on open canvas — used both for a fresh
/// block dropped from the sidebar and as the "create" half of taking an
/// existing block out of a field and dropping it on canvas.
#[tauri::command]
pub(crate) fn create_floating_value<R: Runtime>(
    state: State<SharedState>,
    app: tauri::AppHandle<R>,
    x: i32,
    y: i32,
    value: ValueDto,
) -> Result<String, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    push_undo(&mut s);
    let mac = s.current_macro.as_mut().ok_or("No macro selected")?;
    let id = uuid::Uuid::new_v4().simple().to_string();
    mac.floating_values.push(FloatingValue { id: id.clone(), x, y, value: dto_to_value(&value) });
    auto_save(&s);
    emit_state_updated(&app, &s);
    Ok(id)
}

/// Repositions a floating value block — used when it's dropped on open
/// canvas without landing on any field/subfield slot. No `push_undo`: pure
/// repositioning, same as `move_strand`.
#[tauri::command]
pub(crate) fn move_floating_value<R: Runtime>(
    state: State<SharedState>,
    app: tauri::AppHandle<R>,
    floating_id: String,
    x: i32,
    y: i32,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(mac) = &mut s.current_macro {
        if let Some(fv) = mac.floating_value_mut(&floating_id) {
            fv.x = x;
            fv.y = y;
            auto_save(&s);
        }
    }
    emit_state_updated(&app, &s);
    Ok(())
}

/// Deletes a floating value block outright (dropped on the sidebar trash) —
/// mirrors `remove_strand`.
#[tauri::command]
pub(crate) fn remove_floating_value<R: Runtime>(
    state: State<SharedState>,
    app: tauri::AppHandle<R>,
    floating_id: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    push_undo(&mut s);
    if let Some(mac) = &mut s.current_macro {
        mac.floating_values.retain(|f| f.id != floating_id);
        auto_save(&s);
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

/// Deletes the instruction at `index` in `strand_id`, keeping everything
/// above it in place; anything that was below it is split off into a brand
/// new strand at `(x, y)` (same idea as `split_strand`, but atomic with the
/// removal so it's one undo step). Returns the new strand's id, or `None` if
/// there was nothing below the deleted block to split off.
#[tauri::command]
pub(crate) fn delete_instruction<R: Runtime>(
    state: State<SharedState>,
    app: tauri::AppHandle<R>,
    strand_id: String,
    index: usize,
    x: i32,
    y: i32,
) -> Result<Option<String>, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let in_range = s.current_macro.as_ref()
        .and_then(|mac| mac.strand(&strand_id))
        .is_some_and(|strand| index < strand.instructions.len());
    if !in_range {
        emit_state_updated(&app, &s);
        return Ok(None);
    }
    push_undo(&mut s);
    let mac = s.current_macro.as_mut().ok_or("No macro selected")?;
    let strand = mac.strand_mut(&strand_id).ok_or("Unknown strand")?;
    strand.instructions.remove(index);
    let tail = strand.instructions.split_off(index.min(strand.instructions.len()));
    let now_empty = strand.instructions.is_empty();
    let new_id = if !tail.is_empty() {
        let new_id = uuid::Uuid::new_v4().simple().to_string();
        mac.strands.push(Strand { id: new_id.clone(), x, y, instructions: tail });
        Some(new_id)
    } else {
        None
    };
    // A strand left with no blocks is just dead weight on the canvas — drop
    // it instead of leaving an empty card behind.
    if now_empty {
        mac.strands.retain(|s| s.id != strand_id);
    }
    s.invalid_field_buffers.clear();
    auto_save(&s);
    emit_state_updated(&app, &s);
    Ok(new_id)
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
            // Clearing wipes every strand, including any "When Ran" blocks —
            // matching "start this macro over from scratch". Emptied strands
            // aren't kept around as empty cards; the canvas ends up blank.
            mac.strands.clear();
            s.invalid_field_buffers.clear();
            auto_save(&s);
            s.confirm_clear_instructions = false;
            s.clear_confirm_remaining_secs = 0;
        }
        emit_state_updated(&app, &s);
    }
    Ok(())
}

fn perform_undo<R: Runtime>(state: &SharedState, app: &tauri::AppHandle<R>) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(prev) = s.undo_stack.pop() {
        let current = s.current_macro.as_ref().map(|m| MacroSnapshot { strands: m.strands.clone(), floating_values: m.floating_values.clone() });
        if let Some(cur) = current {
            s.redo_stack.push(cur);
        }
        if let Some(mac) = &mut s.current_macro {
            mac.strands = prev.strands;
            mac.floating_values = prev.floating_values;
            mac.ensure_id();
        }
        s.invalid_field_buffers.clear();
        auto_save(&s);
    }
    emit_state_updated(app, &s);
    Ok(())
}

fn perform_redo<R: Runtime>(state: &SharedState, app: &tauri::AppHandle<R>) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(next) = s.redo_stack.pop() {
        let current = s.current_macro.as_ref().map(|m| MacroSnapshot { strands: m.strands.clone(), floating_values: m.floating_values.clone() });
        if let Some(cur) = current {
            s.undo_stack.push(cur);
        }
        if let Some(mac) = &mut s.current_macro {
            mac.strands = next.strands;
            mac.floating_values = next.floating_values;
            mac.ensure_id();
        }
        s.invalid_field_buffers.clear();
        auto_save(&s);
    }
    emit_state_updated(app, &s);
    Ok(())
}

#[tauri::command]
pub(crate) fn undo<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>) -> Result<(), String> {
    perform_undo(&state, &app)
}

#[tauri::command]
pub(crate) fn redo<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>) -> Result<(), String> {
    perform_redo(&state, &app)
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
        s.invalid_field_buffers.retain(|loc, _| loc.strand_id() != Some(strand_id.as_str()));
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
    s.invalid_field_buffers.retain(|loc, _| loc.strand_id() != Some(dragged_id.as_str()));
    auto_save(&s);
    emit_state_updated(&app, &s);
    Ok(())
}

/// Creates a new detached strand at `(x, y)` holding `instructions` verbatim —
/// how the "Paste" right-click action drops previously-copied blocks onto the
/// canvas. Like `add_strand` but for a whole list of instructions at once.
#[tauri::command]
pub(crate) fn paste_instructions<R: Runtime>(
    state: State<SharedState>,
    app: tauri::AppHandle<R>,
    x: i32,
    y: i32,
    instructions: Vec<InstructionDto>,
) -> Result<String, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let ins: Vec<Instruction> = instructions
        .iter()
        .map(dto_to_instruction)
        .collect::<Option<Vec<_>>>()
        .ok_or("Unknown instruction type")?;
    push_undo(&mut s);
    let mac = s.current_macro.as_mut().ok_or("No macro selected")?;
    let new_id = uuid::Uuid::new_v4().simple().to_string();
    mac.strands.push(Strand { id: new_id.clone(), x, y, instructions: ins });
    auto_save(&s);
    emit_state_updated(&app, &s);
    Ok(new_id)
}

/// Sets the strand that freshly-recorded input is appended to — the "Set
/// Recording Target" right-click action. Not part of the undo/redo stacks
/// (those only snapshot `strands`), so this survives undo/redo untouched, and
/// silently does nothing if `strand_id` doesn't exist (e.g. a stale menu
/// click after a concurrent delete).
#[tauri::command]
pub(crate) fn set_recording_target<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>, strand_id: String) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(mac) = &mut s.current_macro {
        if mac.strand(&strand_id).is_some() {
            mac.recording_target = Some(strand_id);
            auto_save(&s);
        }
    }
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
    let (mac, emulator, is_looping, loop_mode, speed_multiplier) = {
        let s = state.lock().map_err(|e| e.to_string())?;
        let mac = s.current_macro.clone();
        let emulator = s.emulator.as_ref().map(Arc::clone);
        let is_looping = Arc::clone(&s.is_looping);
        let loop_mode = s.loop_mode_enabled;
        let speed_multiplier = mac.as_ref().map_or(1.0, |m| m.speed_multiplier) * s.global_speed_multiplier;
        (mac, emulator, is_looping, loop_mode, speed_multiplier)
    };

    if let (Some(mac), Some(emulator)) = (mac, emulator) {
        if loop_mode {
            if let Err(e) = loop_control::start_loop(&is_looping) {
                warn!("Failed to start loop: {e}");
                return Ok(());
            }
            let mac_name = mac.name.clone();
            let loop_task = mac.into_loop_task(Arc::clone(&emulator), Arc::clone(&is_looping), speed_multiplier);
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
            let single_run_task = mac.into_single_run_task(Arc::clone(&emulator), Arc::clone(&is_looping), speed_multiplier);
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

#[tauri::command]
pub(crate) fn set_global_speed_multiplier<R: Runtime>(state: State<SharedState>, app: tauri::AppHandle<R>, multiplier: f64) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let clamped = multiplier.clamp(*SPEED_MULTIPLIER_RANGE.start(), *SPEED_MULTIPLIER_RANGE.end());
    s.global_speed_multiplier = clamped;
    config::update_settings(|settings| settings.global_speed_multiplier = Some(clamped));
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
            // StopRecording must never require a combo: modifier keys held
            // while it fires would themselves have already been captured as
            // macro steps before the trigger key arrives.
            let combo = if matches!(action, HotkeyAction::StopRecording) {
                KeyCombo { modifiers: 0, ..combo }
            } else {
                combo
            };
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
                let global_speed_multiplier = s.global_speed_multiplier;
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
                    let speed_multiplier = mac.speed_multiplier * global_speed_multiplier;
                    run_macro_task(mac, emulator, is_looping, loop_mode, speed_multiplier);
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
        HotkeyAction::StopRecording => {
            // Only meaningful while a recording is active, which is handled
            // directly in recording::start_grab_thread's capture callback
            // (so it can suppress the trigger key before it's captured as a
            // macro step). Reached here only if pressed while idle — no-op.
        }
        HotkeyAction::Undo => {
            let _ = perform_undo(state, app);
        }
        HotkeyAction::Redo => {
            let _ = perform_redo(state, app);
        }
    }
}

fn run_macro_task(
    mac: Macro,
    emulator: Arc<std::sync::Mutex<dyn crate::macros::backend::InputBackend>>,
    is_looping: Arc<std::sync::Mutex<bool>>,
    loop_mode: bool,
    speed_multiplier: f64,
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
                mac.clone().run(Arc::clone(&emulator), Some(Arc::clone(&loop_flag)), speed_multiplier);
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        });
    } else {
        if let Ok(mut st) = is_looping.lock() { *st = true; }
        let stop_flag = Arc::clone(&is_looping);
        tokio::task::spawn_blocking(move || {
            #[cfg(windows)]
            crate::macros::backend::windows::prepare_for_macro_execution();
            mac.run(emulator, Some(Arc::clone(&stop_flag)), speed_multiplier);
            if let Ok(mut st) = stop_flag.lock() { *st = false; }
        });
    }
}

#[cfg(test)]
mod value_location_tests {
    use super::*;
    use crate::input::types::Coordinate;

    fn test_macro() -> Macro {
        Macro {
            id: "m".into(),
            name: "Test".into(),
            description: "".into(),
            strands: vec![Strand {
                id: "s1".into(),
                x: 0,
                y: 0,
                instructions: vec![
                    Instruction::WhenRan,
                    Instruction::Wait(Value::number(1000.0), Value::number(0.0)),
                ],
            }],
            recording_target: None,
            speed_multiplier: 1.0,
            floating_values: vec![FloatingValue { id: "f1".into(), x: 10, y: 20, value: Value::number(5.0) }],
        }
    }

    #[test]
    fn resolves_field_location() {
        let mut mac = test_macro();
        let loc = ValueLocation::Field { strand_id: "s1".into(), index: 1, field_id: FieldId::WaitDuration, path: vec![] };
        assert_eq!(resolve_location_mut(&mut mac, &loc), Some(&mut Value::number(1000.0)));
    }

    #[test]
    fn resolves_floating_location() {
        let mut mac = test_macro();
        let loc = ValueLocation::Floating { floating_id: "f1".into(), path: vec![] };
        assert_eq!(resolve_location_mut(&mut mac, &loc), Some(&mut Value::number(5.0)));
    }

    #[test]
    fn missing_location_resolves_to_none() {
        let mut mac = test_macro();
        let bad_field = ValueLocation::Field { strand_id: "nope".into(), index: 0, field_id: FieldId::WaitDuration, path: vec![] };
        assert_eq!(resolve_location_mut(&mut mac, &bad_field), None);
        let bad_floating = ValueLocation::Floating { floating_id: "nope".into(), path: vec![] };
        assert_eq!(resolve_location_mut(&mut mac, &bad_floating), None);
    }

    #[test]
    fn apply_value_kind_promotes_leaf_into_operator_lhs() {
        let mut node = Value::number(5.0);
        apply_value_kind(&mut node, "Add").unwrap();
        assert_eq!(node, Value::BinaryOp { op: Op::Add, lhs: Box::new(Value::number(5.0)), rhs: Box::new(Value::number(0.0)) });
    }

    #[test]
    fn apply_value_kind_collapses_operator_to_number_best_effort() {
        let mut node = Value::BinaryOp { op: Op::Add, lhs: Box::new(Value::number(2.0)), rhs: Box::new(Value::number(3.0)) };
        apply_value_kind(&mut node, "Number").unwrap();
        assert_eq!(node, Value::number(5.0));
    }

    #[test]
    fn apply_value_kind_rejects_unknown_kind() {
        let mut node = Value::number(0.0);
        assert!(apply_value_kind(&mut node, "Bogus").is_err());
    }

    #[test]
    fn prune_value_buffers_drops_only_descendants_of_changed_path() {
        let mut buffers: HashMap<ValueLocation, String> = HashMap::new();
        let field = |path: Vec<u8>| ValueLocation::Field { strand_id: "s1".into(), index: 1, field_id: FieldId::WaitDuration, path };
        buffers.insert(field(vec![0]), "kept-sibling-subtree-root".into());
        buffers.insert(field(vec![1]), "dropped-descendant".into());
        buffers.insert(field(vec![1, 0]), "dropped-nested-descendant".into());
        buffers.insert(ValueLocation::Field { strand_id: "s2".into(), index: 1, field_id: FieldId::WaitDuration, path: vec![1] }, "kept-different-strand".into());

        prune_value_buffers(&mut buffers, &field(vec![1]));

        assert_eq!(buffers.len(), 2);
        assert!(buffers.contains_key(&field(vec![0])));
        assert!(buffers.contains_key(&ValueLocation::Field { strand_id: "s2".into(), index: 1, field_id: FieldId::WaitDuration, path: vec![1] }));
    }

    #[test]
    fn location_requires_integer_only_for_pixel_fields() {
        let field = |field_id| ValueLocation::Field { strand_id: "s".into(), index: 0, field_id, path: vec![] };
        assert!(location_requires_integer(&field(FieldId::MoveMouseX)));
        assert!(location_requires_integer(&field(FieldId::ScrollAmount)));
        assert!(!location_requires_integer(&field(FieldId::WaitDuration)));
        assert!(!location_requires_integer(&ValueLocation::Floating { floating_id: "f1".into(), path: vec![] }));
    }

    #[test]
    fn floating_values_round_trip_through_json() {
        let mac = test_macro();
        let json = serde_json::to_string(&mac).unwrap();
        let back: Macro = serde_json::from_str(&json).unwrap();
        assert_eq!(back.floating_values, mac.floating_values);
    }

    #[test]
    fn macro_without_floating_values_key_loads_with_empty_vec() {
        let json = r#"{"id":"m1","name":"Old","description":"","strands":[
            {"id":"s1","x":0,"y":0,"instructions":["WhenRan"]}
        ]}"#;
        let mac: Macro = serde_json::from_str(json).unwrap();
        assert!(mac.floating_values.is_empty());
    }

    #[test]
    fn legacy_bare_number_floating_value_migrates() {
        let json = r#"{"id":"f1","x":10,"y":20,"value":5.0}"#;
        let fv: FloatingValue = serde_json::from_str(json).unwrap();
        assert_eq!(fv.value, Value::number(5.0));
    }

    #[test]
    fn move_mouse_field_still_resolves_after_rework() {
        let mut mac = Macro {
            id: "m".into(),
            name: "T".into(),
            description: "".into(),
            strands: vec![Strand {
                id: "s1".into(),
                x: 0,
                y: 0,
                instructions: vec![Instruction::Token(crate::input::types::InputToken::MoveMouse(
                    Value::number(1.0),
                    Value::number(2.0),
                    Coordinate::Rel,
                ))],
            }],
            recording_target: None,
            speed_multiplier: 1.0,
            floating_values: vec![],
        };
        let loc = ValueLocation::Field { strand_id: "s1".into(), index: 0, field_id: FieldId::MoveMouseY, path: vec![] };
        assert_eq!(resolve_location_mut(&mut mac, &loc), Some(&mut Value::number(2.0)));
    }
}
