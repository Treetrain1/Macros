// Mirrors src-tauri/src/state.rs's StateDto and friends exactly (serde field
// names/shapes are a fixed contract with the Rust backend — do not rename).
import { defaultArgFor, specForKind } from './valueOps';

export type KeyDirection = 'Click' | 'Press' | 'Release';
export type MouseButton = 'Left' | 'Right' | 'Middle' | 'Side' | 'Extra';
export type Coordinate = 'Absolute' | 'Relative';
export type ScrollAxis = 'Vertical' | 'Horizontal';

// A small recursive expression tree backing a value field — a number, text,
// or an operator applied to nested `args` (e.g. `(5) + (3)`). Mirrors
// src-tauri/src/state.rs's ValueDto.
export type ValueOp =
  | 'Add' | 'Sub' | 'Mul' | 'Div' | 'Mod' | 'Round' | 'Random' | 'Join' | 'NewLine' | 'Tab'
  | 'IndexOf' | 'LastIndexOf' | 'LetterOf' | 'Length' | 'Case';
// Join/Join3 both emit op 'Join' (only args.length differs, no 'Join3' on the
// wire). `Var:<name>`/`Param:<name>` are per-variable/per-input identifiers,
// not fixed operators. `Call:<blockId>` (a "My Blocks" reporter) has dynamic
// arity, so it isn't handled by defaultValueForKind/paletteValueFor — see
// blockDefs.ts's paletteCallValueFor.
export type ValueKind = 'Number' | 'Text' | ValueOp | 'Join3' | `Var:${string}` | `Param:${string}` | `Call:${string}`;
// `saved` is the value the operator displaced when it took over the slot —
// carried along so the backend can hand it back if this block is dragged out.
export type ValueDto =
  | { kind: 'Number'; value: number }
  | { kind: 'Text'; value: string }
  | { kind: 'Op'; op: ValueOp; args: ValueDto[]; saved: ValueDto }
  | { kind: 'Var'; name: string }
  | { kind: 'Param'; name: string }
  | { kind: 'Call'; block_id: string; args: ValueDto[]; saved: ValueDto };

export function numberValue(value: number): ValueDto {
  return { kind: 'Number', value };
}

export function textValue(value: string): ValueDto {
  return { kind: 'Text', value };
}

// Fresh default tree for a value block dragged off the sidebar palette —
// mirrors src-tauri/src/commands.rs's apply_value_kind, looked up from
// valueOps.ts's registry so a new operator never needs a new case here.
export function defaultValueForKind(kind: ValueKind): ValueDto {
  if (kind === 'Number') return { kind: 'Number', value: 0 };
  if (kind === 'Text') return { kind: 'Text', value: '' };
  if (kind.startsWith('Var:')) return { kind: 'Var', name: kind.slice('Var:'.length) };
  if (kind.startsWith('Param:')) return { kind: 'Param', name: kind.slice('Param:'.length) };
  // Normally blockDefs.ts's paletteCallValueFor handles `Call:` (it needs the
  // block's input count); this is just a safe zero-arg fallback.
  if (kind.startsWith('Call:')) return { kind: 'Call', block_id: kind.slice('Call:'.length), args: [], saved: numberValue(0) };
  const spec = specForKind(kind);
  if (!spec) throw new Error(`Unknown value kind: ${kind}`);
  return { kind: 'Op', op: spec.op, args: Array.from({ length: spec.arity }, (_, i) => defaultArgFor(spec, i)), saved: numberValue(0) };
}

// Addresses a single Value node: inside an instruction field (Field) or a
// floating canvas block (Floating), at `path` within that root. Mirrors
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
  | { type: 'Text'; text: ValueDto }
  | { type: 'Key'; key: string; direction: KeyDirection }
  | { type: 'Button'; button: MouseButton; direction: KeyDirection }
  | { type: 'MoveMouse'; x: ValueDto; y: ValueDto; coordinate: Coordinate }
  | { type: 'Scroll'; amount: ValueDto; axis: ScrollAxis }
  | { type: 'Command'; command: string }
  | { type: 'Comment'; comment: string }
  | { type: 'WhenRan' }
  | { type: 'SetVariable'; name: string; value: ValueDto }
  | { type: 'ChangeVariable'; name: string; value: ValueDto }
  | { type: 'BlockHeader'; block_id: string }
  | { type: 'CallBlock'; block_id: string; args: ValueDto[] }
  | { type: 'Return'; value: ValueDto };

export type InstructionType = InstructionDto['type'];

// Fresh instruction for a given type — mirrors src-tauri/src/commands.rs's
// defaults. Seeds both a brand-new sidebar drop and a prefab's editable state.
export function defaultInstruction(type: InstructionType): InstructionDto {
  switch (type) {
    case 'WhenRan': return { type: 'WhenRan' };
    case 'Wait': return { type: 'Wait', duration: numberValue(1000) };
    case 'Text': return { type: 'Text', text: textValue('text') };
    case 'Key': return { type: 'Key', key: 'a', direction: 'Click' };
    case 'Button': return { type: 'Button', button: 'Left', direction: 'Click' };
    case 'MoveMouse': return { type: 'MoveMouse', x: numberValue(0), y: numberValue(0), coordinate: 'Relative' };
    case 'Scroll': return { type: 'Scroll', amount: numberValue(4), axis: 'Vertical' };
    case 'Command': return { type: 'Command', command: '' };
    case 'Comment': return { type: 'Comment', comment: '' };
    case 'SetVariable': return { type: 'SetVariable', name: '', value: numberValue(0) };
    case 'ChangeVariable': return { type: 'ChangeVariable', name: '', value: numberValue(0) };
    case 'BlockHeader': return { type: 'BlockHeader', block_id: '' };
    case 'CallBlock': return { type: 'CallBlock', block_id: '', args: [] };
    case 'Return': return { type: 'Return', value: numberValue(0) };
    default: return { type: 'Comment', comment: '' };
  }
}

export const HEADER_TYPES = new Set<InstructionDto['type']>(['WhenRan', 'BlockHeader']);

export function isHeaderType(type: InstructionDto['type']): boolean {
  return HEADER_TYPES.has(type);
}

// "Cap" blocks (the mirror of header blocks) never have anything stacked
// below them — `Return` ends the strand's control flow — so they render
// with a flat bottom edge instead of a connector tab.
export const CAP_TYPES = new Set<InstructionDto['type']>(['Return']);

export function isCapType(type: InstructionDto['type']): boolean {
  return CAP_TYPES.has(type);
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
  /** Declared variable names only — no current-value "watcher" UI. Insertion
   * order; use `sortedVariableNames` for display. */
  variables: string[];
  /** User-defined custom blocks ("My Blocks") — see `BlockDefDto`. */
  block_defs: BlockDefDto[];
}

/** Alphabetical variable names for a macro — shared by the sidebar reporter
 * list and every Set/Change dropdown. */
export function sortedVariableNames(macro: MacroDto | null | undefined): string[] {
  return [...(macro?.variables ?? [])].sort((a, b) => a.localeCompare(b));
}

// One piece of a custom block's prototype, in declaration order — mirrors
// src-tauri/src/macros/mod.rs's BlockPiece. `id` is a stable identifier
// (not the name) so the backend can tell "renamed" apart from "removed +
// added" when reconciling call sites' args on edit_block.
export type BlockPieceDto = { kind: 'Label'; id: string; text: string } | { kind: 'Input'; id: string; name: string };

export interface BlockDefDto {
  id: string;
  pieces: BlockPieceDto[];
  returns_value: boolean;
}

/** A block's declared input names, in prototype order — the positional key
 * `CallBlock`/`Value.Call`'s `args` line up against. */
export function blockInputNames(def: BlockDefDto): string[] {
  return def.pieces.filter((p): p is Extract<BlockPieceDto, { kind: 'Input' }> => p.kind === 'Input').map(p => p.name);
}

/** Looks up a custom block by id in the current macro's `block_defs` — used
 * wherever a call/header needs its prototype, not just its id. */
export function findBlockDef(macro: MacroDto | null | undefined, blockId: string): BlockDefDto | undefined {
  return macro?.block_defs.find(b => b.id === blockId);
}

export interface KeyCaptureDto {
  kind: 'Strand' | 'Standalone';
  strand_id: string | null;
  index: number | null;
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
  standalone_key: string | null;
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
    standalone_key: null,
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
