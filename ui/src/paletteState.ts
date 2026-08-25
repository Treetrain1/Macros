// Ephemeral, client-only state for the sidebar's "prefab" blocks — what an
// instruction/value block looks like while it sits in the palette. Never
// touches the backend or persists, and isn't macro-scoped. Cloned onto the
// canvas via clonePaletteInstruction/paletteValueFor when dragged out.
import { reactive } from 'vue';
import { defaultInstruction, defaultValueForKind, numberValue, textValue } from './types';
import type { InstructionDto, InstructionType, ValueDto, ValueKind } from './types';
import { OPERATOR_KINDS, specForKind } from './valueOps';
import type { OperatorKindSpec, OperatorValueKind } from './valueOps';

const INSTRUCTION_TYPES: InstructionType[] = [
  'WhenRan', 'WhenBatteryDischargedTo', 'WhenBatteryChargedTo', 'Wait', 'Text', 'Key', 'Button', 'MoveMouse', 'Scroll', 'Command', 'Comment',
  'SetVariable', 'ChangeVariable', 'Return', 'If', 'IfElse',
  'Repeat', 'Forever', 'While', 'EscapeLoop', 'ContinueLoop',
];

export const paletteInstructions: Record<InstructionType, InstructionDto> = reactive(
  Object.fromEntries(INSTRUCTION_TYPES.map(t => [t, defaultInstruction(t)])) as Record<InstructionType, InstructionDto>,
);

/** Deep snapshot of a prefab's current state — later palette edits must not
 * retroactively mutate blocks already dropped onto the canvas. */
export function clonePaletteInstruction(type: InstructionType): InstructionDto {
  return JSON.parse(JSON.stringify(paletteInstructions[type]));
}

function argsFor(spec: OperatorKindSpec): (number | string)[] {
  const seed = defaultValueForKind(spec.kind);
  if (seed.kind !== 'Op') return [];
  return seed.args.map(a => (a.kind === 'Number' ? a.value : a.kind === 'Text' ? a.value : ''));
}

// Every operator prefab carries its own editable arg list (numbers or text
// per its `argTypes`) — no nesting, since the sidebar is never a valid drop
// target (see isOverSidebar), so these always stay plain leaves.
export const paletteOperatorArgs: Record<OperatorValueKind, (number | string)[]> = reactive(
  Object.fromEntries(OPERATOR_KINDS.map(s => [s.kind, argsFor(s)])) as Record<OperatorValueKind, (number | string)[]>,
);

const numberSeed = defaultValueForKind('Number');
const textSeed = defaultValueForKind('Text');
export const paletteNumber = reactive({ value: numberSeed.kind === 'Number' ? numberSeed.value : 0 });
export const paletteText = reactive({ value: textSeed.kind === 'Text' ? textSeed.value : '' });

/** The ValueDto a value-palette entry currently represents, built from its
 * live edited state — what lands on the canvas when dragged out. */
export function paletteValueFor(kind: ValueKind): ValueDto {
  if (kind === 'Number') return numberValue(paletteNumber.value);
  if (kind === 'Text') return textValue(paletteText.value);
  if (kind.startsWith('Var:') || kind.startsWith('Param:')) return defaultValueForKind(kind);
  const spec = specForKind(kind);
  if (!spec) throw new Error(`Unknown value kind: ${kind}`);
  const args = paletteOperatorArgs[kind as OperatorValueKind];
  return {
    kind: 'Op',
    op: spec.op,
    args: args.map((v, i) => {
      const argType = spec.argTypes[i];
      if (argType === 'text') return textValue(String(v));
      // No editable palette leaf for booleans — blank, same as
      // defaultArgFor's fallback for a bool-typed slot.
      if (argType === 'bool') return { kind: 'Bool' };
      return numberValue(Number(v));
    }),
    saved: numberValue(0),
  };
}
