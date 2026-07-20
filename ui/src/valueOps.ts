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
  argType: 'number' | 'text';
  /** Rendered before the first arg (word-phrase operators like Random/Join). */
  prefix?: string;
  /** Rendered between each consecutive pair of args, symbol or word alike. */
  infix?: string;
}

export const OPERATOR_KINDS: OperatorKindSpec[] = [
  { kind: 'Add', op: 'Add', arity: 2, argType: 'number', infix: '+' },
  { kind: 'Sub', op: 'Sub', arity: 2, argType: 'number', infix: '−' },
  { kind: 'Mul', op: 'Mul', arity: 2, argType: 'number', infix: '×' },
  { kind: 'Div', op: 'Div', arity: 2, argType: 'number', infix: '/' },
  { kind: 'Random', op: 'Random', arity: 2, argType: 'number', prefix: 'pick random from', infix: 'to' },
  { kind: 'Join', op: 'Join', arity: 2, argType: 'text', prefix: 'join' },
  { kind: 'Join3', op: 'Join', arity: 3, argType: 'text', prefix: 'join' },
  // Zero-arity text constants — argType is unused (no args to render).
  { kind: 'NewLine', op: 'NewLine', arity: 0, argType: 'text', prefix: 'new line' },
  { kind: 'Tab', op: 'Tab', arity: 0, argType: 'text', prefix: 'tab character' },
];

export function specForKind(kind: ValueKind): OperatorKindSpec | undefined {
  return OPERATOR_KINDS.find(s => s.kind === kind);
}

/** Rendering an existing `Op` node only carries `op` (not which palette kind
 * built it) — labels happen to be identical across a given op's arities
 * today (Join and Join3 both say "join"), so this just returns the first
 * spec matching `op`. */
export function labelForOp(op: ValueOp): Pick<OperatorKindSpec, 'prefix' | 'infix'> | undefined {
  const spec = OPERATOR_KINDS.find(s => s.op === op);
  return spec && { prefix: spec.prefix, infix: spec.infix };
}

export function defaultArgFor(spec: OperatorKindSpec): ValueDto {
  return spec.argType === 'text' ? { kind: 'Text', value: '' } : { kind: 'Number', value: 0 };
}
