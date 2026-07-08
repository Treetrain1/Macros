<script setup lang="ts">
import { computed, type Component } from 'vue';
import { ICONS, type IconName } from '../icons';

const props = defineProps<{
  icon?: IconName | Component | null;
  label?: string | null;
}>();

const resolvedIcon = computed<Component | null>(() => {
  if (!props.icon) return null;
  if (typeof props.icon === 'string') return ICONS[props.icon as IconName] ?? null;
  return props.icon as Component;
});
</script>

<template>
  <button type="button">
    <span v-if="resolvedIcon" class="btn-icon-glyph"><component :is="resolvedIcon" /></span>
    <span v-if="label">{{ label }}</span>
  </button>
</template>
