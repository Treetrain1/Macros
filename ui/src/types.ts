// Mirrors src-tauri/src/state.rs's StateDto and friends exactly (serde field
// names/shapes are a fixed contract with the Rust backend — do not rename).

export type KeyDirection = 'Click' | 'Press' | 'Release';
export type MouseButton = 'Left' | 'Right' | 'Middle' | 'Side' | 'Extra';
export type Coordinate = 'Absolute' | 'Relative';
export type ScrollAxis = 'Vertical' | 'Horizontal';

// A small recursive expression tree backing a numeric instruction field —
// either a number, a piece of text, or an operator applied to two nested
// ValueDtos (e.g. `(5) + (3)`). Mirrors src-tauri/src/state.rs's ValueDto.
export type ValueOp = 'Add' | 'Sub' | 'Mul' | 'Div' | 'Random';
export type ValueKind = 'Number' | 'Text' | ValueOp;
// `saved` is whatever value the operator displaced when it took over its
// slot — not a third operand, just carried along so the backend can hand it
// straight back if this operator block is later dragged out of the slot
// (see src-tauri/src/input/value.rs's `Value::BinaryOp`).
export type ValueDto =
  | { kind: 'Number'; value: number }
  | { kind: 'Text'; value: string }
  | { kind: 'BinaryOp'; op: ValueOp; lhs: ValueDto; rhs: ValueDto; saved: ValueDto };

export function numberValue(value: number): ValueDto {
  return { kind: 'Number', value };
}

// Fresh default tree for a value block just dragged off the sidebar palette
// — mirrors src-tauri/src/commands.rs's `apply_value_kind` defaults.
export function defaultValueForKind(kind: ValueKind): ValueDto {
  switch (kind) {
    case 'Number': return { kind: 'Number', value: 0 };
    case 'Text': return { kind: 'Text', value: '' };
    default: return { kind: 'BinaryOp', op: kind, lhs: numberValue(0), rhs: numberValue(0), saved: numberValue(0) };
  }
}

// Addresses a single Value node: either inside an instruction's field
// (Field) or inside a value block parked on canvas (Floating), at `path`
// within that root (0=lhs/1=rhs at each BinaryOp level). Mirrors
// src-tauri/src/state.rs's ValueLocation/ValueLocationDto.
export type ValueLocationDto =
  | { kind: 'Field'; strand_id: string; index: number; field_id: string; path: number[] }
  | { kind: 'Floating'; floating_id: string; path: number[] };

export interface FloatingValueDto {
  id: string;
  x: number;
  y: number;
  value: ValueDto;
}

// Root-of-field location for the field components under ui/src/components/fields/.
export function fieldLocation(strandId: string, index: number, fieldId: string): ValueLocationDto {
  return { kind: 'Field', strand_id: strandId, index, field_id: fieldId, path: [] };
}

export type InstructionDto =
  | { type: 'Wait'; duration: ValueDto }
  | { type: 'Text'; text: string }
  | { type: 'Key'; key: string; direction: KeyDirection }
  | { type: 'Button'; button: MouseButton; direction: KeyDirection }
  | { type: 'MoveMouse'; x: ValueDto; y: ValueDto; coordinate: Coordinate }
  | { type: 'Scroll'; amount: ValueDto; axis: ScrollAxis }
  | { type: 'Command'; command: string }
  | { type: 'Comment'; comment: string }
  | { type: 'WhenRan' };

export type InstructionType = InstructionDto['type'];

// Fresh instruction for a given type — mirrors src-tauri/src/commands.rs's
// instruction defaults. Used both to seed a brand-new drop from the sidebar
// (when the palette hasn't been touched) and, via paletteState.ts, as the
// starting point for a prefab's editable state.
export function defaultInstruction(type: InstructionType): InstructionDto {
  switch (type) {
    case 'WhenRan': return { type: 'WhenRan' };
    case 'Wait': return { type: 'Wait', duration: numberValue(1000) };
    case 'Text': return { type: 'Text', text: 'text' };
    case 'Key': return { type: 'Key', key: 'a', direction: 'Click' };
    case 'Button': return { type: 'Button', button: 'Left', direction: 'Click' };
    case 'MoveMouse': return { type: 'MoveMouse', x: numberValue(0), y: numberValue(0), coordinate: 'Relative' };
    case 'Scroll': return { type: 'Scroll', amount: numberValue(4), axis: 'Vertical' };
    case 'Command': return { type: 'Command', command: '' };
    case 'Comment': return { type: 'Comment', comment: '' };
    default: return { type: 'Comment', comment: '' };
  }
}

export const HEADER_TYPES = new Set<InstructionDto['type']>(['WhenRan']);

export function isHeaderType(type: InstructionDto['type']): boolean {
  return HEADER_TYPES.has(type);
}

export interface StrandDto {
  id: string;
  x: number;
  y: number;
  instructions: InstructionDto[];
}

export interface MacroDto {
  id: string;
  name: string;
  description: string;
  strands: StrandDto[];
  recording_target_strand_id: string | null;
  speed_multiplier: number;
  floating_values: FloatingValueDto[];
}

export interface KeyCaptureDto {
  strand_id: string;
  index: number;
}

export type HotkeyActionDto =
  | { type: 'RunMacro' }
  | { type: 'StopLoop' }
  | { type: 'NextMacro' }
  | { type: 'PrevMacro' }
  | { type: 'ToggleLoop' }
  | { type: 'StartRecordingImmediate' }
  | { type: 'StopRecording' }
  | { type: 'Undo' }
  | { type: 'Redo' }
  | { type: 'RunSpecificMacro'; macro_id: string };

export interface HotkeyBindingDto {
  binding_index: number;
  action: HotkeyActionDto;
  combo_display: string;
  macro_name: string | null;
}

export interface NamedHotkeyDefaultDto {
  action: HotkeyActionDto;
  combo_display: string | null;
}

export interface ComboCaptureDto {
  kind: 'Named' | 'Pending';
  action: HotkeyActionDto | null;
}

export interface PendingMacroHotkeyDto {
  macro_index: number | null;
  combo_display: string | null;
}

export interface InvalidFieldDto {
  location: ValueLocationDto;
  text: string;
}

export type RecordingPhaseName = 'Idle' | 'Countdown' | 'Active';

export interface RecordingPhaseDto {
  phase: RecordingPhaseName;
  countdown: number | null;
}

export type UpdateCheckStateName = 'Idle' | 'Checking' | 'UpToDate' | 'UpdateAvailable' | 'Applying' | 'Error';

export interface UpdateCheckStateDto {
  state: UpdateCheckStateName;
  version: string | null;
  error: string | null;
}

export type PageName = 'Main' | 'Settings';

export interface StateDto {
  macro_names: string[];
  macro_selected: number | null;
  current_macro: MacroDto | null;
  macros_data: MacroDto[];
  loop_mode_enabled: boolean;
  global_speed_multiplier: number;
  is_looping: boolean;
  ipc_active_port: number | null;
  ipc_auto_start: boolean;
  confirm_remove_macro: boolean;
  confirm_remove_macro_remaining_secs: number;
  confirm_clear_instructions: boolean;
  confirm_clear_instructions_remaining_secs: number;
  key_capture: KeyCaptureDto | null;
  can_undo: boolean;
  can_redo: boolean;
  recording_phase: RecordingPhaseDto;
  record_mouse_relative: boolean;
  page: PageName;
  combo_capture: ComboCaptureDto | null;
  hotkey_bindings: HotkeyBindingDto[];
  named_hotkey_defaults: NamedHotkeyDefaultDto[];
  pending_macro_hotkey: PendingMacroHotkeyDto | null;
  invalid_field_buffers: InvalidFieldDto[];
  ipc_port_text: string;
  ipc_port_invalid: boolean;
  emulator_available: boolean;
  grab_available: boolean;
  update_check_state: UpdateCheckStateDto;
}

export function emptyState(): StateDto {
  return {
    macro_names: [],
    macro_selected: null,
    current_macro: null,
    macros_data: [],
    loop_mode_enabled: false,
    global_speed_multiplier: 1.0,
    is_looping: false,
    ipc_active_port: null,
    ipc_auto_start: false,
    confirm_remove_macro: false,
    confirm_remove_macro_remaining_secs: 0,
    confirm_clear_instructions: false,
    confirm_clear_instructions_remaining_secs: 0,
    key_capture: null,
    can_undo: false,
    can_redo: false,
    recording_phase: { phase: 'Idle', countdown: null },
    record_mouse_relative: false,
    page: 'Main',
    combo_capture: null,
    hotkey_bindings: [],
    named_hotkey_defaults: [],
    pending_macro_hotkey: null,
    invalid_field_buffers: [],
    ipc_port_text: '',
    ipc_port_invalid: false,
    emulator_available: true,
    grab_available: true,
    update_check_state: { state: 'Idle', version: null, error: null },
  };
}
