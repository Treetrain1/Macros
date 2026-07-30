// Single source of truth for every operator value-block kind — mirrors
// src-tauri/src/input/value.rs's OPERATOR_KINDS. Add a new operator (or
// arity variant, like Join/Join3) as one row here.
//
// Type-only import below avoids a circular-init hazard with types.ts.
import type { ValueDto, ValueKind, ValueOp } from './types';

/** Every operator `ValueKind` (excludes the `Number`/`Text` leaves) — lets
 * `Record<OperatorValueKind, ...>` state stay in sync automatically. */
export type OperatorValueKind = Exclude<ValueKind, 'Number' | 'Text'>;

export interface OperatorKindSpec {
  kind: OperatorValueKind;
  op: ValueOp;
  arity: number;
  /** One entry per arg, in order — lets an operator mix types (e.g.
   * LetterOf's number-then-text pair). */
  argTypes: ('number' | 'text')[];
  /** Rendered before the first arg (word-phrase operators like Random/Join). */
  prefix?: string;
  /** Rendered between each consecutive pair of args, symbol or word alike. */
  infix?: string;
  /** If set, `args[enumArg.index]` is a fixed dropdown choice, not a
   * draggable Value slot — e.g. Case's upper/lowercase toggle. */
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

/** An existing `Op` node only carries `op`, not which palette kind built it —
 * returns the first spec matching `op` (labels match across arities, e.g. Join/Join3). */
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
