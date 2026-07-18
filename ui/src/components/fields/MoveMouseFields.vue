<script setup lang="ts">
import { editInstruction } from '../../tauri';
import ValueBlock from '../ValueBlock.vue';
import AppDropdown from '../AppDropdown.vue';
import { fieldLocation } from '../../types';
import type { Coordinate, InstructionDto } from '../../types';

const props = defineProps<{ strandId: string; index: number; instruction: Extract<InstructionDto, { type: 'MoveMouse' }> }>();

function onCoordinateChange(v: string) {
  editInstruction(props.strandId, props.index, {
    type: 'MoveMouse', x: props.instruction.x, y: props.instruction.y, coordinate: v as Coordinate,
  });
}
</script>

<template>
  <span class="instruction-label">Move mouse:</span>
  <ValueBlock :location="fieldLocation(strandId, index, 'MoveMouseX')" :value="instruction.x" placeholder="X" />
  <ValueBlock :location="fieldLocation(strandId, index, 'MoveMouseY')" :value="instruction.y" placeholder="Y" />
  <AppDropdown
    :options="['Absolute', 'Relative']"
    :model-value="instruction.coordinate"
    class-name="dd-compact"
    @update:model-value="onCoordinateChange"
  />
</template>
