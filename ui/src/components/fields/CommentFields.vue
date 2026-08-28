<script setup lang="ts">
import { editInstruction } from '../../tauri';
import { AutosizeInput } from 'blockstitch';
import type { InstrPath, InstructionDto } from '../../types';

const props = defineProps<{ strandId: string; path: InstrPath; instruction: Extract<InstructionDto, { type: 'Comment' }> }>();

function onChange(v: string) {
  editInstruction(props.strandId, props.path, { id: props.instruction.id, type: 'Comment', comment: v });
}
</script>

<template>
  <span class="instruction-label">//</span>
  <AutosizeInput
    :model-value="instruction.comment"
    :min-chars="6"
    placeholder="Comment"
    font-style="italic"
    color="var(--blockstitch-text-dim)"
    @update:model-value="onChange"
  />
</template>
