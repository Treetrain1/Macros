<script setup lang="ts">
// Recursive renderer for a `ValueDto` expression tree — a number, a piece of
// text, or an operator block combining two nested `ValueBlock`s (rendered as
// `(lhs) op (rhs)`). Every node, leaf or operator, renders as an actual
// draggable block (`.value-card-shape`, no puzzle-piece connector — these
// never stack in a strand): changing what's in a slot is done by dragging a
// different block from the sidebar's "Operator" section (or an existing
// block) onto it, via valueDrag.ts — there's no in-place kind picker here
// anymore. `location` addresses this node (see invalidField.ts / tauri.ts's
// editValueField/setValueKind/takeValue/putValue); a `<script setup>` SFC
// can reference itself by filename, so no explicit self-registration is
// needed for the recursion below.
import { computed, ref } from 'vue';
import { editValueField } from '../tauri';
import { getInvalidText } from '../invalidField';
import { beginValuePickup } from '../valueDrag';
import AutosizeInput from './AutosizeInput.vue';
import type { ValueDto, ValueLocationDto } from '../types';

const props = defineProps<{
  location: ValueLocationDto;
  value: ValueDto;
  placeholder?: string;
}>();

const OP_SYMBOLS: Record<string, string> = { Add: '+', Sub: '−', Mul: '×', Div: '÷' };

const rootEl = ref<HTMLElement | null>(null);

const buf = computed(() => (props.value.kind === 'Number' ? getInvalidText(props.location) : null));

function childLocation(step: number): ValueLocationDto {
  return { ...props.location, path: [...props.location.path, step] } as ValueLocationDto;
}

function onEdit(text: string) {
  editValueField(props.location, text);
}

function onPointerDown(e: PointerEvent) {
  if ((e.target as Element | null)?.closest?.('input')) return;
  e.stopPropagation();
  if (rootEl.value) beginValuePickup(e, props.location, props.value, rootEl.value);
}
</script>

<template>
  <span
    ref="rootEl"
    class="value-block value-card-shape"
    :data-value-location="JSON.stringify(location)"
    @pointerdown="onPointerDown"
  >
    <template v-if="value.kind === 'BinaryOp'">
      <span class="value-paren">(</span>
      <ValueBlock :location="childLocation(0)" :value="value.lhs" />
      <span class="value-paren">)</span>
      <span class="value-op">{{ OP_SYMBOLS[value.op] }}</span>
      <span class="value-paren">(</span>
      <ValueBlock :location="childLocation(1)" :value="value.rhs" />
      <span class="value-paren">)</span>
    </template>
    <template v-else>
      <AutosizeInput
        :model-value="value.kind === 'Number' ? (buf?.text ?? String(value.value)) : value.value"
        :min-chars="2"
        :invalid="buf?.invalid ?? false"
        :placeholder="placeholder"
        @update:model-value="onEdit"
      />
    </template>
  </span>
</template>
