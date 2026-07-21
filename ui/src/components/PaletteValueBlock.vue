<script setup lang="ts">
// A sidebar "prefab" for a value block (Number/Text literal, any operator
// registered in valueOps.ts's OPERATOR_KINDS, or a `Var:<name>` variable
// reporter) — same boxed .value-card-shape appearance a real
// floating/operator ValueBlock gets (see ValueBlock.vue's `boxed` comment).
// Editable in place via paletteState.ts, but — like PaletteInstructionBlock
// — never recurses and never registers as a value-drop target, so nothing
// (another operator, or a variable) can be dropped into it; the whole
// sidebar already refuses value drops outright (isOverSidebar in
// canvasDrag.ts), so this simply never opts into that machinery.
import { computed } from 'vue';
import type { ValueKind } from '../types';
import { paletteNumber, paletteOperatorArgs, paletteText } from '../paletteState';
import { specForKind } from '../valueOps';
import { beginValuePaletteDrag, paletteEvalPreview } from '../valueDrag';
import { openVariableMenu } from '../contextMenu';
import AutosizeInput from './AutosizeInput.vue';
import AppDropdown from './AppDropdown.vue';

const props = defineProps<{ kind: ValueKind }>();

const spec = computed(() => specForKind(props.kind));
const args = computed(() => (spec.value ? paletteOperatorArgs[spec.value.kind] : []));

// Set only when this exact prefab was last clicked (not dragged) — see
// valueDrag.ts's onPointerUp/previewClickedPaletteValue. Mirrors
// ValueBlock.vue's own `preview` computed, just keyed by `kind` instead of
// a `ValueLocationDto` since a palette prefab isn't a real block anywhere.
const preview = computed(() => (paletteEvalPreview.value?.kind === props.kind ? paletteEvalPreview.value : null));

function onPointerDown(e: PointerEvent) {
  if ((e.target as Element | null)?.closest?.('input, .dd')) return;
  beginValuePaletteDrag(e, props.kind, e.currentTarget as HTMLElement);
}

function onContextMenu(e: MouseEvent) {
  if (!props.kind.startsWith('Var:')) return;
  e.preventDefault();
  openVariableMenu(e, props.kind.slice(4));
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
  if (spec.value.argTypes[index] === 'number') {
    const n = Number(v);
    if (v.trim() !== '' && !isNaN(n)) args.value[index] = n;
  } else {
    args.value[index] = v;
  }
}
</script>

<template>
  <span class="value-block value-card-shape palette-prefab" @pointerdown="onPointerDown" @contextmenu="onContextMenu">
    <template v-if="kind === 'Number'">
      <AutosizeInput :model-value="String(paletteNumber.value)" :min-chars="2" @update:model-value="onNumberInput" />
    </template>
    <template v-else-if="kind === 'Text'">
      <AutosizeInput :model-value="paletteText.value" :min-chars="4" placeholder="text" @update:model-value="onTextInput" />
    </template>
    <template v-else-if="kind.startsWith('Var:')">
      <span class="value-op">{{ kind.slice(4) }}</span>
    </template>
    <template v-else-if="spec">
      <span v-if="spec.prefix" class="value-op">{{ spec.prefix }}</span>
      <template v-for="(arg, i) in args" :key="i">
        <span v-if="i > 0 && spec.infix" class="value-op">{{ spec.infix }}</span>
        <AppDropdown
          v-if="spec.enumArg?.index === i"
          :options="spec.enumArg.options"
          :model-value="String(arg)"
          class-name="dd-compact"
          @update:model-value="v => onArgInput(i, v)"
        />
        <AutosizeInput
          v-else
          :model-value="String(arg)"
          :min-chars="spec.argTypes[i] === 'text' ? 4 : 1"
          :placeholder="spec.argTypes[i] === 'text' ? 'text' : undefined"
          @update:model-value="v => onArgInput(i, v)"
        />
      </template>
    </template>
    <span v-if="preview" class="value-eval-tooltip" :class="{ 'value-eval-tooltip-error': preview.error }">{{ preview.text }}</span>
  </span>
</template>
