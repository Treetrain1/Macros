<script setup lang="ts">
import PaletteNumberField from '../../PaletteNumberField.vue';
import type { InstructionDto } from '../../../types';

const props = defineProps<{ instruction: Extract<InstructionDto, { type: 'Wait' }> }>();
</script>

<template>
  <span class="instruction-label">Wait (ms):</span>
  <PaletteNumberField v-model="props.instruction.duration" />
  <span class="instruction-label">± random:</span>
  <PaletteNumberField v-model="props.instruction.randomness" />
</template>
