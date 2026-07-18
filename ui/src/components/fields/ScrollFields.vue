<script setup lang="ts">
import { editInstruction } from '../../tauri';
import ValueBlock from '../ValueBlock.vue';
import AppDropdown from '../AppDropdown.vue';
import { fieldLocation } from '../../types';
import type { InstructionDto, ScrollAxis } from '../../types';

const props = defineProps<{ strandId: string; index: number; instruction: Extract<InstructionDto, { type: 'Scroll' }> }>();

function onAxisChange(v: string) {
  editInstruction(props.strandId, props.index, { type: 'Scroll', amount: props.instruction.amount, axis: v as ScrollAxis });
}
</script>

<template>
  <span class="instruction-label">Scroll:</span>
  <ValueBlock :location="fieldLocation(strandId, index, 'ScrollAmount')" :value="instruction.amount" />
  <AppDropdown
    :options="['Vertical', 'Horizontal']"
    :model-value="instruction.axis"
    class-name="dd-compact"
    @update:model-value="onAxisChange"
  />
</template>
