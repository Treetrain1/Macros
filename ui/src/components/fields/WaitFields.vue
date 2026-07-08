<script setup lang="ts">
import { computed } from 'vue';
import { editInstructionField } from '../../tauri';
import { getInvalidText } from '../../invalidField';
import AutosizeInput from '../AutosizeInput.vue';
import type { InstructionDto } from '../../types';

const props = defineProps<{ strandId: string; index: number; instruction: Extract<InstructionDto, { type: 'Wait' }> }>();

const durBuf = computed(() => getInvalidText(props.strandId, props.index, 'WaitDuration'));
const randBuf = computed(() => getInvalidText(props.strandId, props.index, 'WaitRandomness'));
</script>

<template>
  <span class="instruction-label">Wait (ms):</span>
  <AutosizeInput
    :model-value="durBuf?.text ?? String(instruction.duration)"
    :min-chars="3"
    :invalid="durBuf?.invalid"
    @update:model-value="v => editInstructionField(strandId, index, 'WaitDuration', v)"
  />
  <span class="instruction-label">± random:</span>
  <AutosizeInput
    :model-value="randBuf?.text ?? String(instruction.randomness)"
    :min-chars="3"
    :invalid="randBuf?.invalid"
    @update:model-value="v => editInstructionField(strandId, index, 'WaitRandomness', v)"
  />
</template>
