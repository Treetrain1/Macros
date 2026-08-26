<script setup lang="ts">
import { Unplug } from 'lucide-vue-next';
import type { InstructionDto } from '../../../types';

// No fields to edit, matches WhenPowerUnpluggedFields.vue.
defineProps<{ instruction: Extract<InstructionDto, { type: 'WhenPowerUnplugged' }> }>();
</script>

<template>
  <Unplug />
  <span class="instruction-label when-ran-label">When Power Unplugged</span>
</template>
