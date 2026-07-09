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

function onPointerUp() {
  if (!dragging.value) return;
  dragging.value = false;

  if (maxAbsDelta.value < 4) {
    const next = !committed.value;
    committed.value = next;
    emit('update:modelValue', next);
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
  <label class="switch" :class="{ 'switch-dragging': dragging }">
    <input type="checkbox" :checked="props.modelValue" readonly>
    <span
      ref="trackEl"
      class="switch-track"
      @pointerdown="onPointerDown"
      @pointermove="onPointerMove"
      @pointerup="onPointerUp"
      @click.prevent
    >
      <span class="switch-knob" :style="knobStyle"></span>
    </span>
    <slot />
  </label>
</template>
