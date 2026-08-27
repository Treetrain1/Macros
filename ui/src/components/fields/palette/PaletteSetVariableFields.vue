<script setup lang="ts">
import { computed } from 'vue';
import { state } from '../../../store';
import { sortedVariableNames } from '../../../types';
import type { InstructionDto } from '../../../types';
import { AppDropdown } from 'blockstitch';
import { PaletteNumberField } from 'blockstitch';

const props = defineProps<{ instruction: Extract<InstructionDto, { type: 'SetVariable' }> }>();

const variableNames = computed(() => sortedVariableNames(state.current_macro));

function onNameChange(name: string) {
  props.instruction.name = name;
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
  <PaletteNumberField v-model="props.instruction.value" />
</template>
