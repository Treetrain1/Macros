<script setup lang="ts">
// Sizes an <input> to its own content in characters (via the `size`
// attribute) instead of a fixed pixel width, so each block is only as wide
// as what's actually typed into it — starting short and growing as the user
// types. The `size` write happens imperatively on the native 'input' event
// (not just reactively off the prop) so it grows immediately as you type,
// without waiting on the backend's confirmation round-trip.
import { onMounted, ref, watch } from 'vue';

const props = withDefaults(defineProps<{
  modelValue: string;
  minChars: number;
  invalid?: boolean;
  placeholder?: string;
  fontStyle?: string;
  color?: string;
}>(), {
  invalid: false,
});

const emit = defineEmits<{ 'update:modelValue': [value: string] }>();

const el = ref<HTMLInputElement | null>(null);

function resize(value: string) {
  if (el.value) el.value.size = Math.max(props.minChars, value.length);
}

onMounted(() => resize(props.modelValue));
watch(() => props.modelValue, v => resize(v));

function onInput(e: Event) {
  const value = (e.target as HTMLInputElement).value;
  resize(value);
  emit('update:modelValue', value);
}
</script>

<template>
  <input
    ref="el"
    type="text"
    :value="modelValue"
    :placeholder="placeholder"
    :class="{ invalid, 'shake-once': invalid }"
    :style="{ fontStyle, color }"
    @input="onInput"
  >
</template>
