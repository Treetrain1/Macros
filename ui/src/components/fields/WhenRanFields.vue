<script setup lang="ts">
import { Play } from 'lucide-vue-next';
import type { InstructionDto } from '../../types';

// No fields to edit — strandId/index/instruction are accepted (like every
// other *Fields component) purely so Vue treats them as declared props
// rather than falling them through as DOM attrs onto the two root nodes
// below.
defineProps<{ strandId: string; index: number; instruction: Extract<InstructionDto, { type: 'WhenRan' }> }>();
</script>

<template>
  <Play />
  <span class="instruction-label when-ran-label">When Ran</span>
</template>
