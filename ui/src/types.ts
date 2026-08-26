// Mirrors src-tauri/src/state.rs's StateDto and friends exactly (serde field
// names/shapes are a fixed contract with the Rust backend — do not rename).
import { defaultArgFor, specForKind } from './valueOps';

export type KeyDirection = 'Click' | 'Press' | 'Release';
export type MouseButton = 'Left' | 'Right' | 'Middle' | 'Side' | 'Extra';
export type Coordinate = 'Absolute' | 'Relative';
export type ScrollAxis = 'Vertical' | 'Horizontal';

export type WeekdayDto = 'Sunday' | 'Monday' | 'Tuesday' | 'Wednesday' | 'Thursday' | 'Friday' | 'Saturday';

// One entry in the "Open App" picker's list — mirrors src-tauri/src/state.rs's
// AppEntryDto. `command` is opaque (platform-specific launch string) and gets
// stored as-is on the `OpenApp` instruction if picked; `icon`, when present,
// is a `data:` URI ready to drop straight into an <img src>.
export interface AppEntryDto {
  name: string;
  command: string;
  icon: string | null;
}

// A recurring point in local time — mirrors macros-core's `TimeSchedule`
// (input/schedule.rs), reused as-is on the wire (same shape as `Op` being
// reused directly in `ValueDto`, not mirrored through a *Dto type). `hour`/
// `minute` are always 24-hour; see timeSchedule.ts for the 12h/24h display
// split and per-kind option lists.
export type TimeScheduleDto =
  | { kind: 'Daily'; hour: number; minute: number }
  | { kind: 'Weekly'; weekday: WeekdayDto; hour: number; minute: number }
  | { kind: 'Monthly'; day: number; hour: number; minute: number }
  | { kind: 'Yearly'; month: number; day: number; hour: number; minute: number };

// A small recursive expression tree backing a value field — a number, text,
// or an operator applied to nested `args` (e.g. `(5) + (3)`). Mirrors
// src-tauri/src/state.rs's ValueDto.
export type ValueOp =
  | 'Add' | 'Sub' | 'Mul' | 'Div' | 'Mod' | 'Round' | 'Random' | 'Join' | 'NewLine' | 'Tab'
  | 'IndexOf' | 'LastIndexOf' | 'LetterOf' | 'Length' | 'Case'
  // Boolean: comparisons, logic, and the two standalone true/false literals
  // (separate blocks, not a toggle — see valueOps.ts's OPERATOR_KINDS).
  | 'Eq' | 'Neq' | 'Gt' | 'Lt' | 'Gte' | 'Lte' | 'And' | 'Or' | 'Not' | 'True' | 'False'
  // Zero-arity — the system's current battery charge, 0-100.
  | 'BatteryPercentage'
  // Zero-arity boolean — whether the system is currently on external power
  // (always true with no battery/UPS present).
  | 'PluggedIn'
  // One fixed-dropdown arg (year/month/date/day of week/hour/minute/second)
  // — always numeric, see timeSchedule.ts's CURRENT_TIME_OPTIONS.
  | 'CurrentTime';
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
  // Bare boolean leaf, no value of its own — the "nothing plugged in here"
  // state of a boolean slot (an If's condition, an And/Or/Not operand).
  // Renders as a blank hexagon (ValueBlock.vue) and evaluates as false.
  | { kind: 'Bool' }
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

// Fresh boolean-typed default — blank, same spirit as
// `numberValue(0)`/`textValue('')`, not a pre-filled "false".
export function blankBoolValue(): ValueDto {
  return { kind: 'Bool' };
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

// One step of an InstrPath — `index` into the current instruction list, and
// (for every step but the last) which nested body of that instruction to
// descend into next: 0 for an If's body or IfElse's `then_body`, 1 for
// IfElse's `else_body`. The last step's `slot` is omitted. Mirrors
// src-tauri/src/state.rs's PathStep.
export interface PathStep {
  index: number;
  slot?: number;
}

// Addresses one instruction, possibly nested inside If/IfElse bodies —
// generalizes a flat instruction index the same way a Value's `path:
// number[]` already addresses a nested value-tree node.
export type InstrPath = PathStep[];

// The common case: a top-level instruction at `index` in a strand's own
// instruction list — what every non-nested call site should pass.
export function topLevelPath(index: number): InstrPath {
  return [{ index }];
}

// Resolves `basePath` against a strand to the (possibly nested) instruction
// list it addresses — a strand's own top-level list for `basePath: []`, or
// an If/IfElse's body for anything longer. Shared by canvasDrag.ts (DOM-less
// placement checks), clipboard.ts, and ContextMenu.vue.
export function resolveInstructionList(strand: StrandDto | null | undefined, basePath: PathStep[]): InstructionDto[] {
  let list: InstructionDto[] | undefined = strand?.instructions;
  for (const step of basePath) {
    const ins = list?.[step.index];
    if (!ins) return [];
    if (ins.type === 'If' && step.slot === 0) list = ins.body;
    else if (ins.type === 'IfElse' && step.slot === 0) list = ins.then_body;
    else if (ins.type === 'IfElse' && step.slot === 1) list = ins.else_body;
    else if (ins.type === 'Repeat' && step.slot === 0) list = ins.body;
    else if (ins.type === 'Forever' && step.slot === 0) list = ins.body;
    else if (ins.type === 'While' && step.slot === 0) list = ins.body;
    else return [];
  }
  return list ?? [];
}

// The single instruction `path` addresses, or `null` if any step along the
// way doesn't resolve (e.g. stale state mid-edit).
export function resolveInstructionAt(strand: StrandDto | null | undefined, path: InstrPath): InstructionDto | null {
  if (path.length === 0) return null;
  const list = resolveInstructionList(strand, path.slice(0, -1));
  return list[path[path.length - 1].index] ?? null;
}

// The path of the instruction immediately after `path`, in the same body
// list — e.g. for "insert a duplicate right after this block."
export function nextSiblingPath(path: InstrPath): InstrPath {
  const steps = [...path];
  const last = steps[steps.length - 1];
  steps[steps.length - 1] = { ...last, index: last.index + 1 };
  return steps;
}

// The base path for an If/IfElse instruction's own nested body — `path` is
// that instruction's own address (its last step has no `slot`, since
// nothing follows it yet); this stamps `slot` onto that last step, so an
// InstructionList rendering the body can append its own children's indices
// after it. `slot` is 0 for an If's body or IfElse's `then_body`, 1 for
// IfElse's `else_body`.
export function bodyBasePath(path: InstrPath, slot: number): InstrPath {
  const steps = [...path];
  const last = steps[steps.length - 1];
  steps[steps.length - 1] = { ...last, slot };
  return steps;
}

// Addresses a single Value node: inside an instruction field (Field) or a
// floating canvas block (Floating), at `path` within that root. Mirrors
// src-tauri/src/state.rs's ValueLocation/ValueLocationDto.
export type ValueLocationDto =
  | { kind: 'Field'; strand_id: string; index: InstrPath; field_id: string; path: number[] }
  | { kind: 'Floating'; floating_id: string; path: number[] };

export interface FloatingValueDto {
  id: string;
  x: number;
  y: number;
  value: ValueDto;
}

// Root-of-field location for the field components under ui/src/components/fields/.
export function fieldLocation(strandId: string, instrPath: InstrPath, fieldId: string): ValueLocationDto {
  return { kind: 'Field', strand_id: strandId, index: instrPath, field_id: fieldId, path: [] };
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
  | { type: 'WhenBatteryDischargedTo'; threshold: ValueDto }
  | { type: 'WhenBatteryChargedTo'; threshold: ValueDto }
  | { type: 'WhenTime'; schedule: TimeScheduleDto }
  | { type: 'WhenPowerPluggedIn' }
  | { type: 'WhenPowerUnplugged' }
  | { type: 'OpenApp'; command: string; name: string; icon: string | null }
  | { type: 'CloseApp'; command: string; name: string; icon: string | null }
  | { type: 'SetVariable'; name: string; value: ValueDto }
  | { type: 'ChangeVariable'; name: string; value: ValueDto }
  | { type: 'BlockHeader'; block_id: string }
  | { type: 'CallBlock'; block_id: string; args: ValueDto[] }
  | { type: 'Return'; value: ValueDto }
  | { type: 'If'; condition: ValueDto; body: InstructionDto[] }
  | { type: 'IfElse'; condition: ValueDto; then_body: InstructionDto[]; else_body: InstructionDto[] }
  | { type: 'Repeat'; count: ValueDto; body: InstructionDto[] }
  | { type: 'Forever'; body: InstructionDto[] }
  | { type: 'While'; condition: ValueDto; body: InstructionDto[] }
  | { type: 'EscapeLoop' }
  | { type: 'ContinueLoop' };

export type InstructionType = InstructionDto['type'];

// Fresh instruction for a given type — mirrors src-tauri/src/commands.rs's
// defaults. Seeds both a brand-new sidebar drop and a prefab's editable state.
export function defaultInstruction(type: InstructionType): InstructionDto {
  switch (type) {
    case 'WhenRan': return { type: 'WhenRan' };
    case 'WhenBatteryDischargedTo': return { type: 'WhenBatteryDischargedTo', threshold: numberValue(20) };
    case 'WhenBatteryChargedTo': return { type: 'WhenBatteryChargedTo', threshold: numberValue(100) };
    case 'WhenTime': return { type: 'WhenTime', schedule: { kind: 'Daily', hour: 9, minute: 0 } };
    case 'WhenPowerPluggedIn': return { type: 'WhenPowerPluggedIn' };
    case 'WhenPowerUnplugged': return { type: 'WhenPowerUnplugged' };
    case 'OpenApp': return { type: 'OpenApp', command: '', name: '', icon: null };
    case 'CloseApp': return { type: 'CloseApp', command: '', name: '', icon: null };
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
    case 'If': return { type: 'If', condition: blankBoolValue(), body: [] };
    case 'IfElse': return { type: 'IfElse', condition: blankBoolValue(), then_body: [], else_body: [] };
    case 'Repeat': return { type: 'Repeat', count: numberValue(10), body: [] };
    case 'Forever': return { type: 'Forever', body: [] };
    case 'While': return { type: 'While', condition: blankBoolValue(), body: [] };
    case 'EscapeLoop': return { type: 'EscapeLoop' };
    case 'ContinueLoop': return { type: 'ContinueLoop' };
    default: return { type: 'Comment', comment: '' };
  }
}

export const HEADER_TYPES = new Set<InstructionDto['type']>(['WhenRan', 'BlockHeader', 'WhenBatteryDischargedTo', 'WhenBatteryChargedTo', 'WhenTime', 'WhenPowerPluggedIn', 'WhenPowerUnplugged']);

export function isHeaderType(type: InstructionDto['type']): boolean {
  return HEADER_TYPES.has(type);
}

// The subset of HEADER_TYPES that are "entry point" triggers (WhenRan and
// every When-condition block) rather than a custom block's own definition
// header (BlockHeader renders its own params instead of a fixed label, so it
// gets its own look — see BlockHeaderFields.vue — not this quiet accent
// tint). Drives `.instruction-row-when-ran`'s styling in InstructionRow.vue/
// PaletteInstructionBlock.vue.
export const ENTRY_TRIGGER_TYPES = new Set<InstructionDto['type']>(['WhenRan', 'WhenBatteryDischargedTo', 'WhenBatteryChargedTo', 'WhenTime', 'WhenPowerPluggedIn', 'WhenPowerUnplugged']);

export function isEntryTriggerType(type: InstructionDto['type']): boolean {
  return ENTRY_TRIGGER_TYPES.has(type);
}

// "Cap" blocks (the mirror of header blocks) never have anything stacked
// below them — `Return` ends the strand's control flow, and `EscapeLoop`/
// `ContinueLoop` jump straight to the enclosing loop's boundary — so they
// render with a flat bottom edge instead of a connector tab.
export const CAP_TYPES = new Set<InstructionDto['type']>(['Return', 'EscapeLoop', 'ContinueLoop']);

export function isCapType(type: InstructionDto['type']): boolean {
  return CAP_TYPES.has(type);
}

// "Wrap"/C-blocks encase a nested body (or two, for If-Else) between their
// own top notch and bottom tab — unlike header/cap, they keep both, since
// they snap above/below like any ordinary block.
export const WRAP_TYPES = new Set<InstructionDto['type']>(['If', 'IfElse', 'Repeat', 'Forever', 'While']);

export function isWrapType(type: InstructionDto['type']): boolean {
  return WRAP_TYPES.has(type);
}

export function hasElseSlot(type: InstructionDto['type']): boolean {
  return type === 'IfElse';
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
  /** Settings edited from the "Macro Settings" popup — see `MacroSettingsDto`. */
  settings: MacroSettingsDto;
}

// Mirrors src-tauri/src/state.rs's MacroSettingsDto — per-macro settings
// edited from the "Macro Settings" popup next to the macro dropdown, and
// included in macro export/import like everything else in MacroDto.
export interface MacroSettingsDto {
  /** When true, this macro's When-Battery/-Time/-Power strands are watched
   * by the background watchers even while a different macro is selected. */
  always_listen: boolean;
}

export function defaultMacroSettings(): MacroSettingsDto {
  return { always_listen: false };
}

/** One non-default `MacroSettingsDto` field an import wants confirmed —
 * mirrors src-tauri/src/state.rs's CustomMacroSettingDto. */
export interface CustomMacroSettingDto {
  key: string;
  label: string;
  enabled: boolean;
}

/** What `importMacro` needs the user to resolve before the staged import can
 * be committed — mirrors src-tauri/src/state.rs's ImportPromptDto. `null`
 * (Rust's `None`) means the import needed no confirmation and already
 * committed. */
export interface ImportPromptDto {
  needs_command_warning: boolean;
  custom_settings: CustomMacroSettingDto[];
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
  index: InstrPath | null;
}

/** Structural equality for two InstrPaths — used wherever a path is
 * compared instead of a bare index (e.g. "is this the row being captured"). */
export function pathsEqual(a: InstrPath | null | undefined, b: InstrPath | null | undefined): boolean {
  if (a == null || b == null) return a === b;
  return a.length === b.length && a.every((step, i) => step.index === b[i].index && step.slot === b[i].slot);
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
  close_to_tray: boolean;
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
    close_to_tray: false,
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
