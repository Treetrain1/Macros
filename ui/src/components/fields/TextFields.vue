<script setup lang="ts">
import { editInstruction } from '../../tauri';
import AutosizeInput from '../AutosizeInput.vue';
import type { InstructionDto } from '../../types';

const props = defineProps<{ strandId: string; index: number; instruction: Extract<InstructionDto, { type: 'Text' }> }>();

function onChange(v: string) {
  editInstruction(props.strandId, props.index, { type: 'Text', text: v });
}
</script>

<template>
  <span class="instruction-label">Text:</span>
  <AutosizeInput :model-value="instruction.text" :min-chars="6" @update:model-value="onChange" />
</template>
