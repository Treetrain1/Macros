// Ephemeral, client-only state for the sidebar's "prefab" blocks — what an
// instruction/value block looks like (and what it's carrying) while it sits
// in the palette. Deliberately never touches the backend: it lives only in
// this module's reactive objects, so it never persists across app restarts
// and is never tied to any particular macro (switching macros can't leak
// into or reset it, because it was never macro-scoped in the first place).
// Whatever a prefab currently holds is exactly what gets cloned onto the
// canvas when it's dragged out — see clonePaletteInstruction/paletteValueFor,
// consumed by canvasDrag.ts and valueDrag.ts at drop time.
import { reactive } from 'vue';
import { defaultInstruction, defaultValueForKind, numberValue } from './types';
import type { InstructionDto, InstructionType, ValueDto, ValueKind } from './types';

const INSTRUCTION_TYPES: InstructionType[] = [
  'WhenRan', 'Wait', 'Text', 'Key', 'Button', 'MoveMouse', 'Scroll', 'Command', 'Comment',
];

export const paletteInstructions: Record<InstructionType, InstructionDto> = reactive(
  Object.fromEntries(INSTRUCTION_TYPES.map(t => [t, defaultInstruction(t)])) as Record<InstructionType, InstructionDto>,
);

/** Deep snapshot of a prefab's current state, safe to hand off to a new
 * strand/instruction — later edits to the palette must not retroactively
 * mutate blocks that were already dropped onto the canvas. */
export function clonePaletteInstruction(type: InstructionType): InstructionDto {
  return JSON.parse(JSON.stringify(paletteInstructions[type]));
}

type OperatorKind = Extract<ValueKind, 'Add' | 'Sub' | 'Mul' | 'Div' | 'Random'>;
const OPERATOR_KINDS: OperatorKind[] = ['Add', 'Sub', 'Mul', 'Div', 'Random'];

function operandsFor(kind: OperatorKind): { lhs: number; rhs: number } {
  const seed = defaultValueForKind(kind);
  const lhs = seed.kind === 'BinaryOp' && seed.lhs.kind === 'Number' ? seed.lhs.value : 0;
  const rhs = seed.kind === 'BinaryOp' && seed.rhs.kind === 'Number' ? seed.rhs.value : 0;
  return { lhs, rhs };
}

// Operator prefabs (Add/Sub/Mul/Div/Random) each carry their own editable lhs/rhs
// pair — there's no nesting support here (dropping another operator onto one
// of these fields would require the sidebar to be a valid drop target, and
// it deliberately isn't, see isOverSidebar), so these always stay plain
// number leaves.
export const paletteOperators: Record<OperatorKind, { lhs: number; rhs: number }> = reactive(
  Object.fromEntries(OPERATOR_KINDS.map(k => [k, operandsFor(k)])) as Record<OperatorKind, { lhs: number; rhs: number }>,
);

const numberSeed = defaultValueForKind('Number');
const textSeed = defaultValueForKind('Text');
export const paletteNumber = reactive({ value: numberSeed.kind === 'Number' ? numberSeed.value : 0 });
export const paletteText = reactive({ value: textSeed.kind === 'Text' ? textSeed.value : '' });

/** The full ValueDto a value-palette entry currently represents, built from
 * its live edited state — what actually lands on the canvas (as a floating
 * value or dropped into a field) when that entry is dragged out. */
export function paletteValueFor(kind: ValueKind): ValueDto {
  if (kind === 'Number') return numberValue(paletteNumber.value);
  if (kind === 'Text') return { kind: 'Text', value: paletteText.value };
  const { lhs, rhs } = paletteOperators[kind];
  return { kind: 'BinaryOp', op: kind, lhs: numberValue(lhs), rhs: numberValue(rhs), saved: numberValue(0) };
}
