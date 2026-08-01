<script setup lang="ts">
import type { InstructionDto } from '../../../types';

// No fields to edit, matches ContinueLoopFields.vue.
defineProps<{ instruction: Extract<InstructionDto, { type: 'ContinueLoop' }> }>();
</script>

<template>
  <span class="instruction-label">continue loop</span>
</template>
