<script setup lang="ts">
import type { StrandDto } from '../types';
import InstructionRow from './InstructionRow.vue';
import { beginPickup } from '../canvasDrag';

const props = defineProps<{ strand: StrandDto }>();

function onEmptyHintPointerDown(e: PointerEvent) {
  beginPickup(e, props.strand.id, 0);
}
</script>

<template>
  <div class="strand-card" :data-strand-id="strand.id">
    <div class="strand-body">
      <div v-if="strand.instructions.length === 0" class="strand-empty-hint" @pointerdown="onEmptyHintPointerDown">
        Empty — drag an instruction here from the sidebar.
      </div>
      <InstructionRow
        v-for="(ins, i) in strand.instructions"
        :key="i"
        :strand-id="strand.id"
        :index="i"
        :instruction="ins"
        :is-first="i === 0"
        :is-last="i === strand.instructions.length - 1"
      />
    </div>
  </div>
</template>
