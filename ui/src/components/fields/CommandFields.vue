<script setup lang="ts">
import { editInstruction } from '../../tauri';
import AutosizeInput from '../AutosizeInput.vue';
import type { InstrPath, InstructionDto } from '../../types';

const props = defineProps<{ strandId: string; path: InstrPath; instruction: Extract<InstructionDto, { type: 'Command' }> }>();

function onChange(v: string) {
  editInstruction(props.strandId, props.path, { id: props.instruction.id, type: 'Command', command: v });
}
</script>

<template>
  <span class="instruction-label">Command:</span>
  <AutosizeInput :model-value="instruction.command" :min-chars="6" placeholder="bash -c …" @update:model-value="onChange" />
</template>
