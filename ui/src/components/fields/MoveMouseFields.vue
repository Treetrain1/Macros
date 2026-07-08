<script setup lang="ts">
import { computed } from 'vue';
import { editInstruction, editInstructionField } from '../../tauri';
import { getInvalidText } from '../../invalidField';
import AutosizeInput from '../AutosizeInput.vue';
import AppDropdown from '../AppDropdown.vue';
import type { Coordinate, InstructionDto } from '../../types';

const props = defineProps<{ strandId: string; index: number; instruction: Extract<InstructionDto, { type: 'MoveMouse' }> }>();

const xBuf = computed(() => getInvalidText(props.strandId, props.index, 'MoveMouseX'));
const yBuf = computed(() => getInvalidText(props.strandId, props.index, 'MoveMouseY'));

function onCoordinateChange(v: string) {
  editInstruction(props.strandId, props.index, {
    type: 'MoveMouse', x: props.instruction.x, y: props.instruction.y, coordinate: v as Coordinate,
  });
}
</script>

<template>
  <span class="instruction-label">Move mouse:</span>
  <AutosizeInput
    :model-value="xBuf?.text ?? String(instruction.x)"
    :min-chars="3"
    :invalid="xBuf?.invalid"
    placeholder="X"
    @update:model-value="v => editInstructionField(strandId, index, 'MoveMouseX', v)"
  />
  <AutosizeInput
    :model-value="yBuf?.text ?? String(instruction.y)"
    :min-chars="3"
    :invalid="yBuf?.invalid"
    placeholder="Y"
    @update:model-value="v => editInstructionField(strandId, index, 'MoveMouseY', v)"
  />
  <AppDropdown
    :options="['Absolute', 'Relative']"
    :model-value="instruction.coordinate"
    class-name="dd-compact"
    @update:model-value="onCoordinateChange"
  />
</template>
