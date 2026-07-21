<script setup lang="ts">
// Recursive renderer for a `ValueDto` expression tree — a number, a piece of
// text, or an operator block combining its nested `args` into `ValueBlock`s
// (rendered as `prefix arg1 infix arg2[ infix arg3 ...]`, labels looked up
// from valueOps.ts's registry — no parens). An operator node, a leaf parked
// at the root of a `Floating` location (a standalone canvas block with
// nothing else to grab it by), or a leaf that landed in its slot by being
// dropped there as its own block (tracked ephemerally by valueDrag.ts's
// capsuleLocations, since the ValueDto itself can't tell a dropped-in leaf
// apart from an ordinary one) renders as an actual bordered,
// pickup-draggable block (`.value-card-shape`); an ordinary leaf sitting in
// a field — never dragged, just typed into — is just its bare input, not
// draggable — see `boxed` below and `onPointerDown`'s gate on it. Changing
// what's in a slot is done by
// dragging a different block from the sidebar's "Operator" section (or an
// existing block) onto it, via valueDrag.ts — there's no in-place kind
// picker here anymore. `location` addresses this node (see invalidField.ts
// / tauri.ts's editValueField/setValueKind/takeValue/putValue); a `<script
// setup>` SFC can reference itself by filename, so no explicit
// self-registration is needed for the recursion below. Clicking (not
// dragging) an operator block samples-evaluates just that node and shows the
// result in a small tooltip — see `preview` below and valueDrag.ts's
// onPointerUp/previewClickedOperator.
import { computed, ref } from 'vue';
import { editValueField } from '../tauri';
import { getInvalidText, locationsEqual } from '../invalidField';
import { beginValuePickup, dragReveal, evalPreview, isCapsuleLocation } from '../valueDrag';
import { labelForOp, specForOp } from '../valueOps';
import AutosizeInput from './AutosizeInput.vue';
import AppDropdown from './AppDropdown.vue';
import type { ValueDto, ValueLocationDto } from '../types';

const props = defineProps<{
  location: ValueLocationDto;
  value: ValueDto;
  placeholder?: string;
}>();

const rootEl = ref<HTMLElement | null>(null);

// While this exact slot is the origin of an in-flight value drag, show what
// take_value will actually leave behind instead of the (about-to-be-taken)
// real value — see valueDrag.ts's dragReveal.
const displayValue = computed(() =>
  dragReveal.value && locationsEqual(dragReveal.value.location, props.location) ? dragReveal.value.value : props.value);

const buf = computed(() => (displayValue.value.kind === 'Number' ? getInvalidText(props.location) : null));

const label = computed(() => (displayValue.value.kind === 'Op' ? labelForOp(displayValue.value.op) : undefined));

// Set for operators with an `enumArg` (e.g. Case's upper/lowercase toggle)
// — that arg renders as a dropdown bound via editValueField, not a nested
// ValueBlock, since it's a fixed choice rather than a draggable value slot.
const enumArg = computed(() => (displayValue.value.kind === 'Op' ? specForOp(displayValue.value.op)?.enumArg : undefined));

function onEnumEdit(step: number, v: string) {
  editValueField(childLocation(step), v);
}

// Set only for the exact operator node that was last clicked (not dragged)
// — see valueDrag.ts's onPointerUp/previewClickedOperator. Auto-clears
// itself after a timeout, so this just reflects whatever's currently live.
const preview = computed(() =>
  evalPreview.value && locationsEqual(evalPreview.value.location, props.location) ? evalPreview.value : null);

const boxed = computed(() =>
  displayValue.value.kind === 'Op' ||
  (props.location.kind === 'Floating' && props.location.path.length === 0) ||
  isCapsuleLocation(props.location));

function childLocation(step: number): ValueLocationDto {
  return { ...props.location, path: [...props.location.path, step] } as ValueLocationDto;
}

function onEdit(text: string) {
  editValueField(props.location, text);
}

function onPointerDown(e: PointerEvent) {
  if ((e.target as Element | null)?.closest?.('input, .dd')) return;
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
    <template v-if="displayValue.kind === 'Op'">
      <span v-if="label?.prefix" class="value-op">{{ label.prefix }}</span>
      <template v-for="(arg, i) in displayValue.args" :key="i">
        <span v-if="i > 0 && label?.infix" class="value-op">{{ label.infix }}</span>
        <AppDropdown
          v-if="enumArg?.index === i"
          :options="enumArg.options"
          :model-value="arg.kind === 'Text' ? arg.value : ''"
          class-name="dd-compact"
          @update:model-value="v => onEnumEdit(i, v)"
        />
        <ValueBlock v-else :location="childLocation(i)" :value="arg" />
      </template>
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
    <span v-if="preview" class="value-eval-tooltip" :class="{ 'value-eval-tooltip-error': preview.error }">{{ preview.text }}</span>
  </span>
</template>
