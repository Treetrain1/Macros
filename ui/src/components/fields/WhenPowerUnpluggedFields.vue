<script setup lang="ts">
import { Unplug } from 'lucide-vue-next';
import type { InstrPath, InstructionDto } from '../../types';

// No fields to edit, matches WhenPowerPluggedInFields.vue/WhenRanFields.vue.
defineProps<{ strandId: string; path: InstrPath; instruction: Extract<InstructionDto, { type: 'WhenPowerUnplugged' }> }>();
</script>

<template>
  <Unplug />
  <span class="instruction-label when-ran-label">When Power Unplugged</span>
</template>
