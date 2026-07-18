<script setup lang="ts">
import { Play } from 'lucide-vue-next';
import type { InstructionDto } from '../../../types';

// No fields to edit, matches WhenRanFields.vue.
defineProps<{ instruction: Extract<InstructionDto, { type: 'WhenRan' }> }>();
</script>

<template>
  <Play />
  <span class="instruction-label when-ran-label">When Ran</span>
</template>
