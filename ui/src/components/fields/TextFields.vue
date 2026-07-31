<script setup lang="ts">
import ValueBlock from '../ValueBlock.vue';
import { fieldLocation } from '../../types';
import type { InstrPath, InstructionDto } from '../../types';

defineProps<{ strandId: string; path: InstrPath; instruction: Extract<InstructionDto, { type: 'Text' }> }>();
</script>

<template>
  <span class="instruction-label">Text:</span>
  <ValueBlock :location="fieldLocation(strandId, path, 'TextValue')" :value="instruction.text" />
</template>
