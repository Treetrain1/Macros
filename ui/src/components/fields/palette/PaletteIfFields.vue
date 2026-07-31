<script setup lang="ts">
// Static preview only — unlike every other *Fields component the condition
// here isn't editable (the palette never recurses into a value tree, see
// PaletteValueBlock.vue's header comment), and the body slot is just a
// placeholder graphic, not a real drop target (dragging this prefab out
// always seeds a fresh empty body via defaultInstruction).
import type { InstructionDto } from '../../../types';

defineProps<{ instruction: Extract<InstructionDto, { type: 'If' }> }>();
</script>

<template>
  <div class="wrap-header">
    <span class="instruction-label">if</span>
    <span class="value-block value-card-shape-bool value-op">false</span>
    <span class="instruction-label">then</span>
  </div>
  <div class="wrap-slot palette-wrap-slot" />
</template>
