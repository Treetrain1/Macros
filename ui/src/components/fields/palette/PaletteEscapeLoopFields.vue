<script setup lang="ts">
import type { InstructionDto } from '../../../types';

// No fields to edit, matches EscapeLoopFields.vue.
defineProps<{ instruction: Extract<InstructionDto, { type: 'EscapeLoop' }> }>();
</script>

<template>
  <span class="instruction-label">escape loop</span>
</template>
