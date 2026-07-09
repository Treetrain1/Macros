// Mirrors src-tauri/src/state.rs's StateDto and friends exactly (serde field
// names/shapes are a fixed contract with the Rust backend — do not rename).

export type KeyDirection = 'Click' | 'Press' | 'Release';
export type MouseButton = 'Left' | 'Right' | 'Middle' | 'Side' | 'Extra';
export type Coordinate = 'Absolute' | 'Relative';
export type ScrollAxis = 'Vertical' | 'Horizontal';

export type InstructionDto =
  | { type: 'Wait'; duration: number; randomness: number }
  | { type: 'Text'; text: string }
  | { type: 'Key'; key: string; direction: KeyDirection }
  | { type: 'Button'; button: MouseButton; direction: KeyDirection }
  | { type: 'MoveMouse'; x: number; y: number; coordinate: Coordinate }
  | { type: 'Scroll'; amount: number; axis: ScrollAxis }
  | { type: 'Command'; command: string }
  | { type: 'Comment'; comment: string }
  | { type: 'WhenRan' };

export type InstructionType = InstructionDto['type'];

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
  | { type: 'RunSpecificMacro'; macro_id: string };

export interface HotkeyBindingDto {
  binding_index: number;
  action: HotkeyActionDto;
  combo_display: string;
  macro_name: string | null;
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
  strand_id: string;
  instruction_index: number;
  field_id: string;
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
    pending_macro_hotkey: null,
    invalid_field_buffers: [],
    ipc_port_text: '',
    ipc_port_invalid: false,
    emulator_available: true,
    grab_available: true,
    update_check_state: { state: 'Idle', version: null, error: null },
  };
}
