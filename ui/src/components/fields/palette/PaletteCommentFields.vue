<script setup lang="ts">
import AutosizeInput from '../../AutosizeInput.vue';
import type { InstructionDto } from '../../../types';

const props = defineProps<{ instruction: Extract<InstructionDto, { type: 'Comment' }> }>();

function onChange(v: string) {
  props.instruction.comment = v;
}
</script>

<template>
  <span class="instruction-label">//</span>
  <AutosizeInput
    :model-value="instruction.comment"
    :min-chars="6"
    placeholder="Comment"
    font-style="italic"
    color="var(--text-dim)"
    @update:model-value="onChange"
  />
</template>
