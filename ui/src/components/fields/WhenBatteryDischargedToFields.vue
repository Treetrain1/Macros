<script setup lang="ts">
import { BatteryWarning } from 'lucide-vue-next';
import { ValueBlock } from 'blockstitch';
import { fieldLocation } from '../../types';
import type { InstrPath, InstructionDto } from '../../types';

defineProps<{ strandId: string; path: InstrPath; instruction: Extract<InstructionDto, { type: 'WhenBatteryDischargedTo' }> }>();
</script>

<template>
  <BatteryWarning />
  <span class="instruction-label when-ran-label">When battery discharged to</span>
  <ValueBlock :location="fieldLocation(strandId, path, 'BatteryDischargeThreshold')" :value="instruction.threshold" />
  <span class="instruction-label">%</span>
</template>
