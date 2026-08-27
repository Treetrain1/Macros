<script setup lang="ts">
import { AutosizeInput } from 'blockstitch';
import type { InstructionDto } from '../../../types';

const props = defineProps<{ instruction: Extract<InstructionDto, { type: 'Command' }> }>();

function onChange(v: string) {
  props.instruction.command = v;
}
</script>

<template>
  <span class="instruction-label">Command:</span>
  <AutosizeInput :model-value="instruction.command" :min-chars="6" placeholder="bash -c …" @update:model-value="onChange" />
</template>
