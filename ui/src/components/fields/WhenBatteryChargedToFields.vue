<script setup lang="ts">
import { BatteryCharging } from 'lucide-vue-next';
import ValueBlock from '../ValueBlock.vue';
import { fieldLocation } from '../../types';
import type { InstrPath, InstructionDto } from '../../types';

defineProps<{ strandId: string; path: InstrPath; instruction: Extract<InstructionDto, { type: 'WhenBatteryChargedTo' }> }>();
</script>

<template>
  <BatteryCharging />
  <span class="instruction-label when-ran-label">When battery charged to</span>
  <ValueBlock :location="fieldLocation(strandId, path, 'BatteryChargeThreshold')" :value="instruction.threshold" />
  <span class="instruction-label">%</span>
</template>
