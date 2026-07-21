// Single source of truth for every operator value-block kind — mirrors
// src-tauri/src/input/value.rs's `OPERATOR_KINDS`. Adding a new operator (or
// a new-arity palette entry for an existing one, like Join/Join3) is one row
// here; every consumer (the sidebar palette list, the palette's default
// argument state, and both ValueBlock.vue/PaletteValueBlock.vue's rendering)
// reads from this table instead of hardcoding its own copy.
//
// Only type-only imports from ./types below — this file has no runtime
// dependency on it, so types.ts (which does depend on this file, for
// defaultValueForKind) can import it back without a circular-init hazard.
import type { ValueDto, ValueKind, ValueOp } from './types';

/** Every `ValueKind` that's an operator (i.e. not a `Number`/`Text` leaf) —
 * derived here so `Record<OperatorValueKind, ...>` state (paletteState.ts)
 * never needs `Number`/`Text` entries and never needs updating by hand. */
export type OperatorValueKind = Exclude<ValueKind, 'Number' | 'Text'>;

export interface OperatorKindSpec {
  kind: OperatorValueKind;
  op: ValueOp;
  arity: number;
  /** One entry per arg, in declaration order — lets an operator mix
   * argument types (e.g. LetterOf's number-then-text pair) instead of
   * assuming every arg is the same kind of leaf. */
  argTypes: ('number' | 'text')[];
  /** Rendered before the first arg (word-phrase operators like Random/Join). */
  prefix?: string;
  /** Rendered between each consecutive pair of args, symbol or word alike. */
  infix?: string;
  /** If set, `args[enumArg.index]` isn't a draggable/typable Value slot but
   * a fixed choice rendered as a dropdown (edited via `editValueField`,
   * never the drag/drop machinery) — e.g. Case's upper/lowercase toggle.
   * See src-tauri/src/input/value.rs's `Op::Case` doc comment. */
  enumArg?: { index: number; options: { value: string; label: string }[] };
}

const CASE_OPTIONS = [
  { value: 'Upper', label: 'uppercase' },
  { value: 'Lower', label: 'lowercase' },
];

export const OPERATOR_KINDS: OperatorKindSpec[] = [
  { kind: 'Add', op: 'Add', arity: 2, argTypes: ['number', 'number'], infix: '+' },
  { kind: 'Sub', op: 'Sub', arity: 2, argTypes: ['number', 'number'], infix: '−' },
  { kind: 'Mul', op: 'Mul', arity: 2, argTypes: ['number', 'number'], infix: '×' },
  { kind: 'Div', op: 'Div', arity: 2, argTypes: ['number', 'number'], infix: '/' },
  { kind: 'Mod', op: 'Mod', arity: 2, argTypes: ['number', 'number'], infix: 'mod' },
  { kind: 'Round', op: 'Round', arity: 1, argTypes: ['number'], prefix: 'round' },
  { kind: 'Random', op: 'Random', arity: 2, argTypes: ['number', 'number'], prefix: 'pick random from', infix: 'to' },
  { kind: 'Join', op: 'Join', arity: 2, argTypes: ['text', 'text'], prefix: 'join' },
  { kind: 'Join3', op: 'Join', arity: 3, argTypes: ['text', 'text', 'text'], prefix: 'join' },
  // Zero-arity text constants — argTypes is unused (no args to render).
  { kind: 'NewLine', op: 'NewLine', arity: 0, argTypes: [], prefix: 'new line' },
  { kind: 'Tab', op: 'Tab', arity: 0, argTypes: [], prefix: 'tab character' },
  { kind: 'IndexOf', op: 'IndexOf', arity: 2, argTypes: ['text', 'text'], prefix: 'index of', infix: 'in' },
  { kind: 'LastIndexOf', op: 'LastIndexOf', arity: 2, argTypes: ['text', 'text'], prefix: 'last index of', infix: 'in' },
  { kind: 'LetterOf', op: 'LetterOf', arity: 2, argTypes: ['number', 'text'], prefix: 'letter', infix: 'of' },
  { kind: 'Length', op: 'Length', arity: 1, argTypes: ['text'], prefix: 'length of' },
  { kind: 'Case', op: 'Case', arity: 2, argTypes: ['text', 'text'], infix: 'to', enumArg: { index: 1, options: CASE_OPTIONS } },
];

export function specForKind(kind: ValueKind): OperatorKindSpec | undefined {
  return OPERATOR_KINDS.find(s => s.kind === kind);
}

/** Rendering an existing `Op` node only carries `op` (not which palette kind
 * built it) — labels happen to be identical across a given op's arities
 * today (Join and Join3 both say "join"), so this just returns the first
 * spec matching `op`. */
export function specForOp(op: ValueOp): OperatorKindSpec | undefined {
  return OPERATOR_KINDS.find(s => s.op === op);
}

export function labelForOp(op: ValueOp): Pick<OperatorKindSpec, 'prefix' | 'infix'> | undefined {
  const spec = specForOp(op);
  return spec && { prefix: spec.prefix, infix: spec.infix };
}

export function defaultArgFor(spec: OperatorKindSpec, index: number): ValueDto {
  if (spec.enumArg?.index === index) return { kind: 'Text', value: spec.enumArg.options[0].value };
  return spec.argTypes[index] === 'text' ? { kind: 'Text', value: '' } : { kind: 'Number', value: 0 };
}
