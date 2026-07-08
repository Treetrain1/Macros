<script setup lang="ts">
import { onMounted, watch } from 'vue';
import { state } from '../store';
import { attachDragListeners, positionCanvas } from '../canvasDrag';
import StrandCard from './StrandCard.vue';

defineProps<{ recording: boolean }>();

onMounted(() => {
  attachDragListeners();
  positionCanvas(state.current_macro);
});

// DOM geometry (bounding box measurement + card left/top/canvas
// width/height/transform) is an inherently two-pass, DOM-measurement
// operation — it needs to read the just-rendered, unstyled DOM
// (offsetWidth/offsetHeight), which Vue's reactivity graph has no visibility
// into. So this runs as a plain post-patch pass rather than a computed/:style
// binding: `flush: 'post'` guarantees the v-for'd .strand-card elements
// already exist in the DOM when it runs.
watch(
  () => state.current_macro?.strands,
  () => positionCanvas(state.current_macro),
  { flush: 'post', deep: true },
);
</script>

<template>
  <div class="canvas-wrap">
    <div class="canvas-scroll" id="canvas-scroll">
      <div class="canvas-sizer" id="canvas-sizer">
        <div class="canvas-inner" id="canvas-inner">
          <StrandCard v-for="strand in state.current_macro?.strands ?? []" :key="strand.id" :strand="strand" />
        </div>
      </div>
    </div>
    <div id="recording-overlay" class="recording-overlay" v-show="recording">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="6" fill="currentColor"></circle></svg>
      <span>Recording…</span>
      <span class="recording-overlay-hint">Adding and removing instructions is disabled while recording.</span>
    </div>
  </div>
</template>
