<script setup lang="ts">
import { BatteryCharging } from 'lucide-vue-next';
import PaletteNumberField from '../../PaletteNumberField.vue';
import type { InstructionDto } from '../../../types';

const props = defineProps<{ instruction: Extract<InstructionDto, { type: 'WhenBatteryChargedTo' }> }>();
</script>

<template>
  <BatteryCharging />
  <span class="instruction-label when-ran-label">When battery charged to</span>
  <PaletteNumberField v-model="props.instruction.threshold" />
  <span class="instruction-label">%</span>
</template>
