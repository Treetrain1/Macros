<script setup lang="ts">
import ValueBlock from '../ValueBlock.vue';
import { fieldLocation } from '../../types';
import type { InstrPath, InstructionDto } from '../../types';

defineProps<{ strandId: string; path: InstrPath; instruction: Extract<InstructionDto, { type: 'Return' }> }>();
</script>

<template>
  <span class="instruction-label">return</span>
  <ValueBlock :location="fieldLocation(strandId, path, 'ReturnValue')" :value="instruction.value" />
</template>
