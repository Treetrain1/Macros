<script setup lang="ts">
// A sidebar "prefab" for one instruction type — visually a real
// .instruction-row/.instruction-shape (see style.css), bound to this type's
// live entry in paletteState.ts so it renders exactly what would spawn onto
// the canvas right now. Its fields are genuinely editable (see the
// fields/palette/* components), but editing never touches the backend —
// there's no real strand/index behind a palette entry — and dragging always
// starts from this block itself (no separate hidden ghost template needed,
// since this *is* the block: the drag ghost is just a clone of it).
import { computed, type Component } from 'vue';
import type { InstructionDto, InstructionType } from '../types';
import { isHeaderType } from '../types';
import { paletteInstructions } from '../paletteState';
import { beginPaletteDrag } from '../canvasDrag';
import { ICONS, INSTRUCTION_TYPE_ICONS } from '../icons';
import PaletteWhenRanFields from './fields/palette/PaletteWhenRanFields.vue';
import PaletteWaitFields from './fields/palette/PaletteWaitFields.vue';
import PaletteTextFields from './fields/palette/PaletteTextFields.vue';
import PaletteKeyFields from './fields/palette/PaletteKeyFields.vue';
import PaletteButtonFields from './fields/palette/PaletteButtonFields.vue';
import PaletteMoveMouseFields from './fields/palette/PaletteMoveMouseFields.vue';
import PaletteScrollFields from './fields/palette/PaletteScrollFields.vue';
import PaletteCommandFields from './fields/palette/PaletteCommandFields.vue';
import PaletteCommentFields from './fields/palette/PaletteCommentFields.vue';

const props = defineProps<{ type: InstructionType }>();

const FIELD_COMPONENTS: Record<InstructionDto['type'], Component> = {
  WhenRan: PaletteWhenRanFields,
  Wait: PaletteWaitFields,
  Text: PaletteTextFields,
  Key: PaletteKeyFields,
  Button: PaletteButtonFields,
  MoveMouse: PaletteMoveMouseFields,
  Scroll: PaletteScrollFields,
  Command: PaletteCommandFields,
  Comment: PaletteCommentFields,
};

const instruction = computed(() => paletteInstructions[props.type]);
const fieldComponent = computed(() => FIELD_COMPONENTS[props.type]);
const typeIcon = computed(() => ICONS[INSTRUCTION_TYPE_ICONS[props.type]]);

function onPointerDown(e: PointerEvent) {
  const target = e.target as Element | null;
  if (target?.closest?.('input, select, textarea, button, .dd-trigger, .dd-option')) return;
  if (target instanceof HTMLElement && target.isContentEditable) return;
  const el = e.currentTarget as HTMLElement;
  beginPaletteDrag(e, props.type, el.cloneNode(true) as HTMLElement);
}
</script>

<template>
  <div
    class="instruction-row palette-prefab"
    :class="{ 'instruction-row-when-ran': type === 'WhenRan', 'instruction-row-header': isHeaderType(type) }"
    @pointerdown="onPointerDown"
  >
    <div class="instruction-shape">
      <component :is="typeIcon" v-if="!isHeaderType(type)" class="instruction-type-icon" />
      <div class="instruction-content">
        <component :is="fieldComponent" :instruction="instruction" />
      </div>
    </div>
  </div>
</template>
