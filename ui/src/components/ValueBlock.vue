<script setup lang="ts">
// Recursive renderer for a `ValueDto` expression tree — a number, a piece of
// text, or an operator block combining two nested `ValueBlock`s (rendered as
// `lhs op rhs`, no parens — or `prefix lhs infix rhs` for word-phrase
// operators like Random, see OP_LABELS below). An operator node, a leaf
// parked at the root of a `Floating` location (a standalone canvas block
// with nothing else to grab it by), or a leaf that landed in its slot by
// being dropped there as its own block (tracked ephemerally by
// valueDrag.ts's capsuleLocations, since the ValueDto itself can't tell a
// dropped-in leaf apart from an ordinary one) renders as an actual bordered,
// pickup-draggable block (`.value-card-shape`); an ordinary leaf sitting in
// a field — never dragged, just typed into — is just its bare input, not
// draggable — see `boxed` below and `onPointerDown`'s gate on it. Changing
// what's in a slot is done by
// dragging a different block from the sidebar's "Operator" section (or an
// existing block) onto it, via valueDrag.ts — there's no in-place kind
// picker here anymore. `location` addresses this node (see invalidField.ts
// / tauri.ts's editValueField/setValueKind/takeValue/putValue); a `<script
// setup>` SFC can reference itself by filename, so no explicit
// self-registration is needed for the recursion below.
import { computed, ref } from 'vue';
import { editValueField } from '../tauri';
import { getInvalidText, locationsEqual } from '../invalidField';
import { beginValuePickup, dragReveal, isCapsuleLocation } from '../valueDrag';
import AutosizeInput from './AutosizeInput.vue';
import type { ValueDto, ValueLocationDto } from '../types';

const props = defineProps<{
  location: ValueLocationDto;
  value: ValueDto;
  placeholder?: string;
}>();

// `prefix` renders before lhs (for word-phrase operators like Random);
// `infix` sits between lhs and rhs, symbol or word alike.
const OP_LABELS: Record<string, { prefix?: string; infix: string }> = {
  Add: { infix: '+' },
  Sub: { infix: '−' },
  Mul: { infix: '×' },
  Div: { infix: '/' },
  Random: { prefix: 'pick random from', infix: 'to' },
};

const rootEl = ref<HTMLElement | null>(null);

// While this exact slot is the origin of an in-flight value drag, show what
// take_value will actually leave behind instead of the (about-to-be-taken)
// real value — see valueDrag.ts's dragReveal.
const displayValue = computed(() =>
  dragReveal.value && locationsEqual(dragReveal.value.location, props.location) ? dragReveal.value.value : props.value);

const buf = computed(() => (displayValue.value.kind === 'Number' ? getInvalidText(props.location) : null));

const boxed = computed(() =>
  displayValue.value.kind === 'BinaryOp' ||
  (props.location.kind === 'Floating' && props.location.path.length === 0) ||
  isCapsuleLocation(props.location));

function childLocation(step: number): ValueLocationDto {
  return { ...props.location, path: [...props.location.path, step] } as ValueLocationDto;
}

function onEdit(text: string) {
  editValueField(props.location, text);
}

function onPointerDown(e: PointerEvent) {
  if ((e.target as Element | null)?.closest?.('input')) return;
  // A bare leaf sitting in a field is just its input's padding — nothing to
  // grab, so don't let a click there start an extraction drag; only an
  // actual block (operator, floating root, or a dropped-in capsule leaf —
  // see `boxed`) is pickup-draggable.
  if (!boxed.value) return;
  e.stopPropagation();
  if (rootEl.value) beginValuePickup(e, props.location, props.value, rootEl.value);
}
</script>

<template>
  <span
    ref="rootEl"
    class="value-block"
    :class="{ 'value-card-shape': boxed }"
    :data-value-location="JSON.stringify(location)"
    @pointerdown="onPointerDown"
  >
    <template v-if="displayValue.kind === 'BinaryOp'">
      <span v-if="OP_LABELS[displayValue.op].prefix" class="value-op">{{ OP_LABELS[displayValue.op].prefix }}</span>
      <ValueBlock :location="childLocation(0)" :value="displayValue.lhs" />
      <span class="value-op">{{ OP_LABELS[displayValue.op].infix }}</span>
      <ValueBlock :location="childLocation(1)" :value="displayValue.rhs" />
    </template>
    <template v-else>
      <AutosizeInput
        :model-value="displayValue.kind === 'Number' ? (buf?.text ?? String(displayValue.value)) : displayValue.value"
        :min-chars="2"
        :invalid="buf?.invalid ?? false"
        :placeholder="placeholder"
        @update:model-value="onEdit"
      />
    </template>
  </span>
</template>
