<script setup lang="ts">
import { PlugZap } from 'lucide-vue-next';
import type { InstructionDto } from '../../../types';

// No fields to edit, matches WhenPowerPluggedInFields.vue.
defineProps<{ instruction: Extract<InstructionDto, { type: 'WhenPowerPluggedIn' }> }>();
</script>

<template>
  <PlugZap />
  <span class="instruction-label when-ran-label">When Power Plugged In</span>
</template>
