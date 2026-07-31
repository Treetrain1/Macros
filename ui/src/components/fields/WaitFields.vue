<script setup lang="ts">
import ValueBlock from '../ValueBlock.vue';
import { fieldLocation } from '../../types';
import type { InstrPath, InstructionDto } from '../../types';

defineProps<{ strandId: string; path: InstrPath; instruction: Extract<InstructionDto, { type: 'Wait' }> }>();
</script>

<template>
  <span class="instruction-label">Wait (ms):</span>
  <ValueBlock :location="fieldLocation(strandId, path, 'WaitDuration')" :value="instruction.duration" />
</template>
