<script setup lang="ts">
// Static preview only — see PaletteIfFields.vue's header comment.
import type { InstructionDto } from '../../../types';

defineProps<{ instruction: Extract<InstructionDto, { type: 'IfElse' }> }>();
</script>

<template>
  <div class="wrap-header">
    <span class="instruction-label">if</span>
    <span class="value-block value-card-shape-bool value-op">false</span>
    <span class="instruction-label">then</span>
  </div>
  <div class="wrap-slot palette-wrap-slot" />
  <div class="wrap-divider">
    <span class="instruction-label">else</span>
  </div>
  <div class="wrap-slot palette-wrap-slot" />
</template>
