<script setup lang="ts">
import { PaletteNumberField } from 'blockstitch';
import type { InstructionDto } from '../../../types';

const props = defineProps<{ instruction: Extract<InstructionDto, { type: 'Return' }> }>();
</script>

<template>
  <span class="instruction-label">return</span>
  <PaletteNumberField v-model="props.instruction.value" />
</template>
