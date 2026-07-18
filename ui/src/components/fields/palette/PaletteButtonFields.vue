<script setup lang="ts">
import AppDropdown from '../../AppDropdown.vue';
import type { InstructionDto, KeyDirection, MouseButton } from '../../../types';

const props = defineProps<{ instruction: Extract<InstructionDto, { type: 'Button' }> }>();

function onButtonChange(v: string) {
  props.instruction.button = v as MouseButton;
}
function onDirectionChange(dir: string) {
  props.instruction.direction = dir as KeyDirection;
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
