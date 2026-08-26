<script setup lang="ts">
import { onMounted, watch } from 'vue';
import { state } from '../store';
import { attachDragListeners, positionCanvas, resetCanvasView, zoomInCanvas, zoomOutCanvas } from '../canvasDrag';
import { attachValueDragListeners } from '../valueDrag';
import { openCanvasMenu } from '../contextMenu';
import StrandCard from './StrandCard.vue';
import FloatingValueCard from './FloatingValueCard.vue';
import CommentCard from './CommentCard.vue';
import ContextMenu from './ContextMenu.vue';

defineProps<{ recording: boolean }>();

// Only genuine empty-space right-clicks reach here — InstructionRow's own
// contextmenu handler calls stopPropagation for right-clicks on a block.
function onCanvasContextMenu(e: MouseEvent) {
  openCanvasMenu(e);
}

onMounted(() => {
  attachDragListeners();
  attachValueDragListeners();
  positionCanvas(state.current_macro);
});

// DOM geometry (bounding box measurement + card left/top/canvas
// width/height/transform) is an inherently two-pass, DOM-measurement
// operation — it needs to read the just-rendered, unstyled DOM
// (offsetWidth/offsetHeight), which Vue's reactivity graph has no visibility
// into. So this runs as a plain post-patch pass rather than a computed/:style
// binding: `flush: 'post'` guarantees the v-for'd .strand-card/
// .value-floating-card elements already exist in the DOM when it runs.
watch(
  () => [state.current_macro?.strands, state.current_macro?.floating_values, state.current_macro?.comments],
  () => positionCanvas(state.current_macro),
  { flush: 'post', deep: true },
);
</script>

<template>
  <div class="canvas-wrap">
    <div class="canvas-scroll" id="canvas-scroll" @contextmenu.prevent="onCanvasContextMenu">
      <div class="canvas-sizer" id="canvas-sizer">
        <div class="canvas-inner" id="canvas-inner">
          <svg id="comment-connector-layer" class="comment-connector-layer"></svg>
          <StrandCard v-for="strand in state.current_macro?.strands ?? []" :key="strand.id" :strand="strand" />
          <FloatingValueCard
            v-for="fv in state.current_macro?.floating_values ?? []"
            :key="fv.id"
            :floating-value="fv"
          />
          <CommentCard
            v-for="comment in state.current_macro?.comments ?? []"
            :key="comment.id"
            :comment="comment"
          />
        </div>
      </div>
    </div>
    <div id="recording-overlay" class="recording-overlay" v-show="recording">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="6" fill="currentColor"></circle></svg>
      <span>Recording…</span>
      <span class="recording-overlay-hint">Adding and removing instructions is disabled while recording.</span>
    </div>
    <div class="canvas-zoom-controls">
      <button type="button" class="canvas-zoom-btn" title="Zoom in" @click="zoomInCanvas">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="11" cy="11" r="7"></circle>
          <line x1="11" y1="8" x2="11" y2="14"></line>
          <line x1="8" y1="11" x2="14" y2="11"></line>
          <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
        </svg>
      </button>
      <button type="button" class="canvas-zoom-btn" title="Zoom out" @click="zoomOutCanvas">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="11" cy="11" r="7"></circle>
          <line x1="8" y1="11" x2="14" y2="11"></line>
          <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
        </svg>
      </button>
      <button type="button" class="canvas-zoom-btn" title="Reset view" @click="resetCanvasView">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"></path>
          <path d="M3 3v5h5"></path>
        </svg>
      </button>
    </div>
    <ContextMenu />
  </div>
</template>
