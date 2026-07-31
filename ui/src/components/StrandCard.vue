<script setup lang="ts">
import type { StrandDto } from '../types';
import InstructionList from './InstructionList.vue';

defineProps<{ strand: StrandDto }>();
</script>

<template>
  <div class="strand-card" :data-strand-id="strand.id">
    <InstructionList :strand-id="strand.id" :base-path="[]" :instructions="strand.instructions" />
  </div>
</template>
