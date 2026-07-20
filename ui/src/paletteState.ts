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
import { defaultInstruction, defaultValueForKind, numberValue, textValue } from './types';
import type { InstructionDto, InstructionType, ValueDto, ValueKind } from './types';
import { OPERATOR_KINDS, specForKind } from './valueOps';
import type { OperatorKindSpec, OperatorValueKind } from './valueOps';

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

function argsFor(spec: OperatorKindSpec): (number | string)[] {
  const seed = defaultValueForKind(spec.kind);
  if (seed.kind !== 'Op') return [];
  return seed.args.map(a => (a.kind === 'Number' ? a.value : a.kind === 'Text' ? a.value : ''));
}

// Every operator prefab (Add/Sub/Mul/Div/Random/Join/Join3, and whatever's
// added to valueOps.ts's OPERATOR_KINDS next) carries its own editable arg
// list, numbers or text per its `argType` — there's no nesting support here
// (dropping another operator onto one of these fields would require the
// sidebar to be a valid drop target, and it deliberately isn't, see
// isOverSidebar), so these always stay plain leaves.
export const paletteOperatorArgs: Record<OperatorValueKind, (number | string)[]> = reactive(
  Object.fromEntries(OPERATOR_KINDS.map(s => [s.kind, argsFor(s)])) as Record<OperatorValueKind, (number | string)[]>,
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
  if (kind === 'Text') return textValue(paletteText.value);
  const spec = specForKind(kind);
  if (!spec) throw new Error(`Unknown value kind: ${kind}`);
  const args = paletteOperatorArgs[kind as OperatorValueKind];
  return {
    kind: 'Op',
    op: spec.op,
    args: args.map(v => (spec.argType === 'text' ? textValue(String(v)) : numberValue(Number(v)))),
    saved: numberValue(0),
  };
}
