<script setup lang="ts">
import { computed } from 'vue';
import { Play } from 'lucide-vue-next';
import type { StrandDto } from '../types';
import InstructionRow from './InstructionRow.vue';
import { beginPickup, ROOT_ID } from '../canvasDrag';

const props = defineProps<{ strand: StrandDto }>();
const isRoot = computed(() => props.strand.id === ROOT_ID);

function onEmptyHintPointerDown(e: PointerEvent) {
  if (!isRoot.value) beginPickup(e, props.strand.id, 0);
}
</script>

<template>
  <div class="strand-card" :class="{ 'is-root': isRoot }" :data-strand-id="strand.id">
    <div class="strand-body">
      <div v-if="isRoot" class="root-marker">
        <Play />
        <span>Root</span>
      </div>
      <div v-if="strand.instructions.length === 0" class="strand-empty-hint" @pointerdown="onEmptyHintPointerDown">
        Empty — drag an instruction here from the sidebar.
      </div>
      <InstructionRow
        v-for="(ins, i) in strand.instructions"
        :key="i"
        :strand-id="strand.id"
        :index="i"
        :instruction="ins"
      />
    </div>
  </div>
</template>
