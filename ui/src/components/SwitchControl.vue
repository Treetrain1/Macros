<script setup lang="ts">
import { ref, computed, watch } from 'vue';

const props = defineProps<{
  modelValue: boolean;
}>();

const emit = defineEmits<{
  'update:modelValue': [value: boolean];
}>();

const dragging = ref(false);
const dragX = ref(0);
const startClientX = ref(0);
const startKnobX = ref(0);
const maxAbsDelta = ref(0);

const committed = ref(props.modelValue);

watch(() => props.modelValue, (v) => {
  committed.value = v;
});

const knobStyle = computed(() => ({
  transform: `translateX(${dragging.value ? dragX.value : (committed.value ? 16 : 0)}px)`
}));

const trackEl = ref<HTMLElement | null>(null);

function onPointerDown(e: PointerEvent) {
  committed.value = props.modelValue;
  dragging.value = true;
  trackEl.value?.setPointerCapture(e.pointerId);
  startClientX.value = e.clientX;
  startKnobX.value = props.modelValue ? 16 : 0;
  dragX.value = startKnobX.value;
  maxAbsDelta.value = 0;
}

function onPointerMove(e: PointerEvent) {
  if (!dragging.value) return;
  const delta = e.clientX - startClientX.value;
  maxAbsDelta.value = Math.max(maxAbsDelta.value, Math.abs(delta));
  dragX.value = Math.max(0, Math.min(16, startKnobX.value + delta));
}

function toggle() {
  const next = !committed.value;
  committed.value = next;
  emit('update:modelValue', next);
}

function onPointerUp() {
  if (!dragging.value) return;
  dragging.value = false;

  if (maxAbsDelta.value < 4) {
    toggle();
    return;
  }

  const next = dragX.value > 8;
  if (next !== committed.value) {
    committed.value = next;
    emit('update:modelValue', next);
  }
}
</script>

<template>
  <!-- A click landing on the slotted label text (not the track) has nothing
       of ours to catch it locally, so it bubbles up to this label — which
       wraps the checkbox and would otherwise trigger the browser's own
       "clicking a label toggles its control" behavior, flipping the
       checkbox's real DOM `checked` state (and, via the CSS `:checked`
       selector below, its color) without going through `toggle()` at all.
       `.prevent` here cancels that native forwarding so `toggle()` — which
       the track's own click already stops from bubbling this far — is the
       only thing that ever changes the switch's on/off appearance. -->
  <label class="switch" :class="{ 'switch-dragging': dragging }" @click.prevent="toggle">
    <input type="checkbox" :checked="props.modelValue" readonly>
    <span
      ref="trackEl"
      class="switch-track"
      @pointerdown="onPointerDown"
      @pointermove="onPointerMove"
      @pointerup="onPointerUp"
      @click.stop.prevent
    >
      <span class="switch-knob" :style="knobStyle"></span>
    </span>
    <slot />
  </label>
</template>
