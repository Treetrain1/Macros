<script setup lang="ts">
import ValueBlock from '../ValueBlock.vue';
import { fieldLocation } from '../../types';
import type { InstructionDto } from '../../types';

defineProps<{ strandId: string; index: number; instruction: Extract<InstructionDto, { type: 'Text' }> }>();
</script>

<template>
  <span class="instruction-label">Text:</span>
  <ValueBlock :location="fieldLocation(strandId, index, 'TextValue')" :value="instruction.text" />
</template>
