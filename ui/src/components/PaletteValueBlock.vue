<script setup lang="ts">
// A sidebar "prefab" for a value block (Number/Text literal, an
// Add/Sub/Mul/Div/Random operator, or a Join/Join3 text-concatenation
// operator) — same boxed .value-card-shape appearance a real
// floating/operator ValueBlock gets (see ValueBlock.vue's `boxed` comment).
// Editable in place via paletteState.ts, but — like PaletteInstructionBlock —
// never recurses and never registers as a value-drop target, so nothing
// (another operator, and eventually a variable) can be dropped into it; the
// whole sidebar already refuses value drops outright (isOverSidebar in
// canvasDrag.ts), so this simply never opts into that machinery.
import type { ValueKind } from '../types';
import { paletteJoins, paletteNumber, paletteOperators, paletteText } from '../paletteState';
import { beginValuePaletteDrag } from '../valueDrag';
import AutosizeInput from './AutosizeInput.vue';

const props = defineProps<{ kind: ValueKind }>();

const OP_LABELS: Record<string, { prefix?: string; infix: string }> = {
  Add: { infix: '+' },
  Sub: { infix: '−' },
  Mul: { infix: '×' },
  Div: { infix: '/' },
  Random: { prefix: 'pick random from', infix: 'to' },
};

function isJoinKind(kind: ValueKind): kind is 'Join' | 'Join3' {
  return kind === 'Join' || kind === 'Join3';
}

function onPointerDown(e: PointerEvent) {
  if ((e.target as Element | null)?.closest?.('input')) return;
  beginValuePaletteDrag(e, props.kind, e.currentTarget as HTMLElement);
}

function onNumberInput(v: string) {
  const n = Number(v);
  if (v.trim() !== '' && !isNaN(n)) paletteNumber.value = n;
}
function onTextInput(v: string) {
  paletteText.value = v;
}
function onOperandInput(side: 'lhs' | 'rhs', kind: ValueKind, v: string) {
  if (kind === 'Number' || kind === 'Text' || isJoinKind(kind)) return;
  const n = Number(v);
  if (v.trim() !== '' && !isNaN(n)) paletteOperators[kind][side] = n;
}
function onJoinArgInput(kind: ValueKind, index: number, v: string) {
  if (!isJoinKind(kind)) return;
  paletteJoins[kind][index] = v;
}
</script>

<template>
  <span class="value-block value-card-shape palette-prefab" @pointerdown="onPointerDown">
    <template v-if="kind === 'Number'">
      <AutosizeInput :model-value="String(paletteNumber.value)" :min-chars="2" @update:model-value="onNumberInput" />
    </template>
    <template v-else-if="kind === 'Text'">
      <AutosizeInput :model-value="paletteText.value" :min-chars="4" placeholder="text" @update:model-value="onTextInput" />
    </template>
    <template v-else-if="isJoinKind(kind)">
      <span class="value-op">join</span>
      <AutosizeInput
        v-for="(arg, i) in paletteJoins[kind]"
        :key="i"
        :model-value="arg"
        :min-chars="4"
        placeholder="text"
        @update:model-value="v => onJoinArgInput(kind, i, v)"
      />
    </template>
    <template v-else>
      <span v-if="OP_LABELS[kind].prefix" class="value-op">{{ OP_LABELS[kind].prefix }}</span>
      <AutosizeInput
        :model-value="String(paletteOperators[kind].lhs)"
        :min-chars="1"
        @update:model-value="v => onOperandInput('lhs', kind, v)"
      />
      <span class="value-op">{{ OP_LABELS[kind].infix }}</span>
      <AutosizeInput
        :model-value="String(paletteOperators[kind].rhs)"
        :min-chars="1"
        @update:model-value="v => onOperandInput('rhs', kind, v)"
      />
    </template>
  </span>
</template>
