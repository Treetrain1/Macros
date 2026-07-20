<script setup lang="ts">
// A sidebar "prefab" for a value block (Number/Text literal, or any operator
// registered in valueOps.ts's OPERATOR_KINDS) — same boxed .value-card-shape
// appearance a real floating/operator ValueBlock gets (see ValueBlock.vue's
// `boxed` comment). Editable in place via paletteState.ts, but — like
// PaletteInstructionBlock — never recurses and never registers as a
// value-drop target, so nothing (another operator, and eventually a
// variable) can be dropped into it; the whole sidebar already refuses value
// drops outright (isOverSidebar in canvasDrag.ts), so this simply never
// opts into that machinery.
import { computed } from 'vue';
import type { ValueKind } from '../types';
import { paletteNumber, paletteOperatorArgs, paletteText } from '../paletteState';
import { specForKind } from '../valueOps';
import { beginValuePaletteDrag } from '../valueDrag';
import AutosizeInput from './AutosizeInput.vue';

const props = defineProps<{ kind: ValueKind }>();

const spec = computed(() => specForKind(props.kind));
const args = computed(() => (spec.value ? paletteOperatorArgs[spec.value.kind] : []));

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
function onArgInput(index: number, v: string) {
  if (!spec.value) return;
  if (spec.value.argType === 'number') {
    const n = Number(v);
    if (v.trim() !== '' && !isNaN(n)) args.value[index] = n;
  } else {
    args.value[index] = v;
  }
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
    <template v-else-if="spec">
      <span v-if="spec.prefix" class="value-op">{{ spec.prefix }}</span>
      <template v-for="(arg, i) in args" :key="i">
        <span v-if="i > 0 && spec.infix" class="value-op">{{ spec.infix }}</span>
        <AutosizeInput
          :model-value="String(arg)"
          :min-chars="spec.argType === 'text' ? 4 : 1"
          :placeholder="spec.argType === 'text' ? 'text' : undefined"
          @update:model-value="v => onArgInput(i, v)"
        />
      </template>
    </template>
  </span>
</template>
