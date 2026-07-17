import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getVersion } from '@tauri-apps/api/app';
import type { HotkeyActionDto, InstructionDto, StateDto } from './types';

export function getState(): Promise<StateDto> {
  return invoke('get_state');
}

export function onStateUpdated(cb: (state: StateDto) => void): Promise<() => void> {
  return listen<StateDto>('state-updated', evt => cb(evt.payload));
}

export function getAppVersion(): Promise<string> {
  return getVersion();
}

// ─── Macro CRUD ─────────────────────────────────────────────────────────────
export const selectMacro = (index: number) => invoke<void>('select_macro', { index });
export const newMacro = () => invoke<void>('new_macro');
export const removeMacro = () => invoke<void>('remove_macro');
export const setTitle = (title: string) => invoke<void>('set_title', { title });
export const setMacroSpeedMultiplier = (multiplier: number) =>
  invoke<void>('set_macro_speed_multiplier', { multiplier });
export const saveMacro = () => invoke<void>('save_macro');

// ─── Instructions ───────────────────────────────────────────────────────────
export const addInstruction = (strandId: string, index: number, instruction: InstructionDto) =>
  invoke<void>('add_instruction', { strandId, index, instruction });
export const editInstruction = (strandId: string, index: number, instruction: InstructionDto) =>
  invoke<void>('edit_instruction', { strandId, index, instruction });
export const editInstructionField = (strandId: string, index: number, fieldId: string, text: string) =>
  invoke<void>('edit_instruction_field', { strandId, index, fieldId, text });
export const removeInstruction = (strandId: string, index: number) =>
  invoke<void>('remove_instruction', { strandId, index });
export const reorderInstruction = (strandId: string, index: number, direction: number) =>
  invoke<void>('reorder_instruction', { strandId, index, direction });
export const clearInstructions = () => invoke<void>('clear_instructions');

// ─── Undo/redo ──────────────────────────────────────────────────────────────
export const undo = () => invoke<void>('undo');
export const redo = () => invoke<void>('redo');

// ─── Strands ────────────────────────────────────────────────────────────────
export const addStrand = (x: number | null, y: number | null, instruction: InstructionDto | null) =>
  invoke<string>('add_strand', { x, y, instruction });
export const removeStrand = (strandId: string) => invoke<void>('remove_strand', { strandId });
export const moveStrand = (strandId: string, x: number, y: number) =>
  invoke<void>('move_strand', { strandId, x, y });
export const splitStrand = (strandId: string, index: number, x: number, y: number) =>
  invoke<string>('split_strand', { strandId, index, x, y });
export const mergeStrand = (draggedId: string, targetId: string, index: number) =>
  invoke<void>('merge_strand', { draggedId, targetId, index });
export const deleteInstruction = (strandId: string, index: number, x: number, y: number) =>
  invoke<string | null>('delete_instruction', { strandId, index, x, y });
export const pasteInstructions = (x: number, y: number, instructions: InstructionDto[]) =>
  invoke<string>('paste_instructions', { x, y, instructions });
export const setRecordingTarget = (strandId: string) =>
  invoke<void>('set_recording_target', { strandId });

// ─── Key capture ────────────────────────────────────────────────────────────
export const startKeyCapture = (strandId: string, index: number) =>
  invoke<void>('start_key_capture', { strandId, index });
export const keyCaptureEvent = (code: string, key: string) =>
  invoke<void>('key_capture_event', { code, key });

// ─── Run / record / loop ────────────────────────────────────────────────────
export const runMacro = () => invoke<void>('run_macro');
export const toggleLoopMode = (enabled: boolean) => invoke<void>('toggle_loop_mode', { enabled });
export const setGlobalSpeedMultiplier = (multiplier: number) =>
  invoke<void>('set_global_speed_multiplier', { multiplier });
export const startRecording = () => invoke<void>('start_recording');
export const stopRecording = () => invoke<void>('stop_recording');
export const toggleRecordMouseRelative = (relative: boolean) =>
  invoke<void>('toggle_record_mouse_relative', { relative });

// ─── Settings navigation ────────────────────────────────────────────────────
export const openSettings = () => invoke<void>('open_settings');
export const closeSettings = () => invoke<void>('close_settings');

// ─── Hotkeys ────────────────────────────────────────────────────────────────
export const startComboCapture = (action: HotkeyActionDto) =>
  invoke<void>('start_combo_capture', { action });
export const startPendingComboCapture = () => invoke<void>('start_pending_combo_capture');
export const comboCaptureEvent = (code: string, modifiers: number) =>
  invoke<void>('combo_capture_event', { code, modifiers });
export const cancelComboCapture = () => invoke<void>('cancel_combo_capture');
export const setPendingMacroIdx = (index: number | null) =>
  invoke<void>('set_pending_macro_idx', { index });
export const addMacroHotkey = () => invoke<void>('add_macro_hotkey');
export const removeHotkeyBinding = (index: number) => invoke<void>('remove_hotkey_binding', { index });
export const clearNamedHotkey = (action: HotkeyActionDto) => invoke<void>('clear_named_hotkey', { action });
export const resetHotkeyToDefault = (action: HotkeyActionDto) =>
  invoke<void>('reset_hotkey_to_default', { action });

// ─── TCP / IPC server ───────────────────────────────────────────────────────
export const setIpcPortText = (text: string) => invoke<void>('set_ipc_port_text', { text });
export const startIpcServer = () => invoke<void>('start_ipc_server');
export const stopIpcServer = () => invoke<void>('stop_ipc_server');
export const setIpcAutoStart = (enabled: boolean) => invoke<void>('set_ipc_auto_start', { enabled });

// ─── Updates ────────────────────────────────────────────────────────────────
export const checkForUpdates = () => invoke<void>('check_for_updates');
export const applyUpdate = () => invoke<void>('apply_update');
