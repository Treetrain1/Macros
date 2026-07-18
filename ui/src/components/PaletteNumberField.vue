<script setup lang="ts">
// A palette instruction's numeric field (Wait duration/randomness, MoveMouse
// x/y, Scroll amount) — same bare-leaf appearance as a real ValueBlock leaf
// (see ValueBlock.vue's `boxed` comment), but deliberately not a drag
// source/drop target: sidebar prefabs never carry operators or (future)
// variables, only literal numbers, and the whole sidebar already refuses
// value drops (see canvasDrag.ts's isOverSidebar), so this component simply
// never wires up that machinery in the first place.
import { computed } from 'vue';
import AutosizeInput from './AutosizeInput.vue';
import { numberValue } from '../types';
import type { ValueDto } from '../types';

const props = defineProps<{ modelValue: ValueDto; placeholder?: string }>();
const emit = defineEmits<{ 'update:modelValue': [ValueDto] }>();

const text = computed(() => String(props.modelValue.kind === 'Number' ? props.modelValue.value : 0));

function onInput(v: string) {
  const n = Number(v);
  if (v.trim() !== '' && !isNaN(n)) emit('update:modelValue', numberValue(n));
}
</script>

<template>
  <span class="value-block">
    <AutosizeInput :model-value="text" :min-chars="2" :placeholder="placeholder" @update:model-value="onInput" />
  </span>
</template>
