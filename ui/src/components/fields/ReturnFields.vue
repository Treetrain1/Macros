<script setup lang="ts">
import ValueBlock from '../ValueBlock.vue';
import { fieldLocation } from '../../types';
import type { InstructionDto } from '../../types';

defineProps<{ strandId: string; index: number; instruction: Extract<InstructionDto, { type: 'Return' }> }>();
</script>

<template>
  <span class="instruction-label">return</span>
  <ValueBlock :location="fieldLocation(strandId, index, 'ReturnValue')" :value="instruction.value" />
</template>
