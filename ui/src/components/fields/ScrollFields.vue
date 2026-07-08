<script setup lang="ts">
import { computed } from 'vue';
import { editInstruction, editInstructionField } from '../../tauri';
import { getInvalidText } from '../../invalidField';
import AutosizeInput from '../AutosizeInput.vue';
import AppDropdown from '../AppDropdown.vue';
import type { InstructionDto, ScrollAxis } from '../../types';

const props = defineProps<{ strandId: string; index: number; instruction: Extract<InstructionDto, { type: 'Scroll' }> }>();

const amtBuf = computed(() => getInvalidText(props.strandId, props.index, 'ScrollAmount'));

function onAxisChange(v: string) {
  editInstruction(props.strandId, props.index, { type: 'Scroll', amount: props.instruction.amount, axis: v as ScrollAxis });
}
</script>

<template>
  <span class="instruction-label">Scroll:</span>
  <AutosizeInput
    :model-value="amtBuf?.text ?? String(instruction.amount)"
    :min-chars="3"
    :invalid="amtBuf?.invalid"
    @update:model-value="v => editInstructionField(strandId, index, 'ScrollAmount', v)"
  />
  <AppDropdown
    :options="['Vertical', 'Horizontal']"
    :model-value="instruction.axis"
    class-name="dd-compact"
    @update:model-value="onAxisChange"
  />
</template>
