<script setup lang="ts">
import { PaletteNumberField } from 'blockstitch';
import { AppDropdown } from 'blockstitch';
import type { InstructionDto, ScrollAxis } from '../../../types';

const props = defineProps<{ instruction: Extract<InstructionDto, { type: 'Scroll' }> }>();

function onAxisChange(v: string) {
  props.instruction.axis = v as ScrollAxis;
}
</script>

<template>
  <span class="instruction-label">Scroll:</span>
  <PaletteNumberField v-model="props.instruction.amount" />
  <AppDropdown
    :options="['Vertical', 'Horizontal']"
    :model-value="instruction.axis"
    class-name="dd-compact"
    @update:model-value="onAxisChange"
  />
</template>
