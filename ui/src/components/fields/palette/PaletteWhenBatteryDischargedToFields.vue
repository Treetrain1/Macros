<script setup lang="ts">
import { BatteryWarning } from 'lucide-vue-next';
import { PaletteNumberField } from 'blockstitch';
import type { InstructionDto } from '../../../types';

const props = defineProps<{ instruction: Extract<InstructionDto, { type: 'WhenBatteryDischargedTo' }> }>();
</script>

<template>
  <BatteryWarning />
  <span class="instruction-label when-ran-label">When battery discharged to</span>
  <PaletteNumberField v-model="props.instruction.threshold" />
  <span class="instruction-label">%</span>
</template>
