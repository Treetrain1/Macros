<script setup lang="ts">
import type { FloatingValueDto } from '../types';
import ValueBlock from './ValueBlock.vue';

defineProps<{ floatingValue: FloatingValueDto }>();
</script>

<template>
  <div class="value-floating-card" :data-floating-id="floatingValue.id">
    <ValueBlock :location="{ kind: 'Floating', floating_id: floatingValue.id, path: [] }" :value="floatingValue.value" />
  </div>
</template>
