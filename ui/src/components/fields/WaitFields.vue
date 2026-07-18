<script setup lang="ts">
import ValueBlock from '../ValueBlock.vue';
import { fieldLocation } from '../../types';
import type { InstructionDto } from '../../types';

defineProps<{ strandId: string; index: number; instruction: Extract<InstructionDto, { type: 'Wait' }> }>();
</script>

<template>
  <span class="instruction-label">Wait (ms):</span>
  <ValueBlock :location="fieldLocation(strandId, index, 'WaitDuration')" :value="instruction.duration" />
  <span class="instruction-label">± random:</span>
  <ValueBlock :location="fieldLocation(strandId, index, 'WaitRandomness')" :value="instruction.randomness" />
</template>
