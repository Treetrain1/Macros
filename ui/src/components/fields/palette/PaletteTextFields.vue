<script setup lang="ts">
import AutosizeInput from '../../AutosizeInput.vue';
import type { InstructionDto } from '../../../types';

const props = defineProps<{ instruction: Extract<InstructionDto, { type: 'Text' }> }>();

function onChange(v: string) {
  props.instruction.text = v;
}
</script>

<template>
  <span class="instruction-label">Text:</span>
  <AutosizeInput :model-value="instruction.text" :min-chars="6" @update:model-value="onChange" />
</template>
