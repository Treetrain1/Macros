<script setup lang="ts">
import { editInstruction } from '../../tauri';
import AppDropdown from '../AppDropdown.vue';
import type { InstructionDto, KeyDirection, MouseButton } from '../../types';

const props = defineProps<{ strandId: string; index: number; instruction: Extract<InstructionDto, { type: 'Button' }> }>();

function onButtonChange(v: string) {
  editInstruction(props.strandId, props.index, { type: 'Button', button: v as MouseButton, direction: props.instruction.direction });
}
function onDirectionChange(dir: string) {
  editInstruction(props.strandId, props.index, { type: 'Button', button: props.instruction.button, direction: dir as KeyDirection });
}
</script>

<template>
  <span class="instruction-label">Mouse:</span>
  <AppDropdown
    :options="['Left', 'Right', 'Middle', 'Side', 'Extra']"
    :model-value="instruction.button"
    class-name="dd-compact"
    @update:model-value="onButtonChange"
  />
  <AppDropdown
    :options="['Click', 'Press', 'Release']"
    :model-value="instruction.direction"
    class-name="dd-compact"
    @update:model-value="onDirectionChange"
  />
</template>
