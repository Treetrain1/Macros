<script setup lang="ts">
// Static, single-line preview only — see PaletteIfFields.vue's header comment.
import type { InstructionDto } from '../../../types';

defineProps<{ instruction: Extract<InstructionDto, { type: 'IfElse' }> }>();
</script>

<template>
  <span class="instruction-label">if</span>
  <span class="value-block value-card-shape-bool value-op">false</span>
  <span class="instruction-label">then … else …</span>
</template>
