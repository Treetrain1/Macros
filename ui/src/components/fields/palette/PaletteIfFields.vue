<script setup lang="ts">
// Static, single-line preview only — the real canvas block is a hollow
// bracket shape (see style.css's .wrap-head/.wrap-spine/.wrap-foot), but a
// narrow sidebar chip just shows what it is, not a live editable body.
import type { InstructionDto } from '../../../types';

defineProps<{ instruction: Extract<InstructionDto, { type: 'If' }> }>();
</script>

<template>
  <span class="instruction-label">if</span>
  <span class="value-block value-card-shape-bool value-op">false</span>
  <span class="instruction-label">then …</span>
</template>
