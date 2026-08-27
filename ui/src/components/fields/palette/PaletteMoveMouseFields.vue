<script setup lang="ts">
import { PaletteNumberField } from 'blockstitch';
import { AppDropdown } from 'blockstitch';
import type { Coordinate, InstructionDto } from '../../../types';

const props = defineProps<{ instruction: Extract<InstructionDto, { type: 'MoveMouse' }> }>();

function onCoordinateChange(v: string) {
  props.instruction.coordinate = v as Coordinate;
}
</script>

<template>
  <span class="instruction-label">Move mouse:</span>
  <PaletteNumberField v-model="props.instruction.x" placeholder="X" />
  <PaletteNumberField v-model="props.instruction.y" placeholder="Y" />
  <AppDropdown
    :options="['Absolute', 'Relative']"
    :model-value="instruction.coordinate"
    class-name="dd-compact"
    @update:model-value="onCoordinateChange"
  />
</template>
