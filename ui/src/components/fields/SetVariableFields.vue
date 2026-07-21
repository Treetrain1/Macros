<script setup lang="ts">
import { computed } from 'vue';
import { state } from '../../store';
import { editInstruction } from '../../tauri';
import { fieldLocation, sortedVariableNames } from '../../types';
import type { InstructionDto } from '../../types';
import AppDropdown from '../AppDropdown.vue';
import ValueBlock from '../ValueBlock.vue';

const props = defineProps<{ strandId: string; index: number; instruction: Extract<InstructionDto, { type: 'SetVariable' }> }>();

const variableNames = computed(() => sortedVariableNames(state.current_macro));

function onNameChange(name: string) {
  editInstruction(props.strandId, props.index, { type: 'SetVariable', name, value: props.instruction.value });
}
</script>

<template>
  <span class="instruction-label">set</span>
  <AppDropdown
    :options="variableNames"
    :model-value="instruction.name"
    placeholder="variable"
    class-name="dd-compact"
    @update:model-value="onNameChange"
  />
  <span class="instruction-label">to</span>
  <ValueBlock :location="fieldLocation(strandId, index, 'SetVariableValue')" :value="instruction.value" />
</template>
