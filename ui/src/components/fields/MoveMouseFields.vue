<script setup lang="ts">
import { editInstruction } from '../../tauri';
import ValueBlock from '../ValueBlock.vue';
import AppDropdown from '../AppDropdown.vue';
import { fieldLocation } from '../../types';
import type { Coordinate, InstrPath, InstructionDto } from '../../types';

const props = defineProps<{ strandId: string; path: InstrPath; instruction: Extract<InstructionDto, { type: 'MoveMouse' }> }>();

function onCoordinateChange(v: string) {
  editInstruction(props.strandId, props.path, {
    type: 'MoveMouse', x: props.instruction.x, y: props.instruction.y, coordinate: v as Coordinate,
  });
}
</script>

<template>
  <span class="instruction-label">Move mouse:</span>
  <ValueBlock :location="fieldLocation(strandId, path, 'MoveMouseX')" :value="instruction.x" placeholder="X" />
  <ValueBlock :location="fieldLocation(strandId, path, 'MoveMouseY')" :value="instruction.y" placeholder="Y" />
  <AppDropdown
    :options="['Absolute', 'Relative']"
    :model-value="instruction.coordinate"
    class-name="dd-compact"
    @update:model-value="onCoordinateChange"
  />
</template>
