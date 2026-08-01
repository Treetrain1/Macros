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
import { isCapType, isHeaderType, isWrapType } from '../types';
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
import PaletteSetVariableFields from './fields/palette/PaletteSetVariableFields.vue';
import PaletteChangeVariableFields from './fields/palette/PaletteChangeVariableFields.vue';
import PaletteReturnFields from './fields/palette/PaletteReturnFields.vue';
import PaletteIfFields from './fields/palette/PaletteIfFields.vue';
import PaletteIfElseFields from './fields/palette/PaletteIfElseFields.vue';

// `BlockHeader`/`CallBlock` are excluded — a header is never dragged from
// the sidebar at all (only ever created via the "Make a Block" dialog), and
// a call's prototype is per-block-id/dynamic (see PaletteCallBlock.vue in
// the "My Blocks" section) rather than one fixed shape this generic prefab
// could render.
const props = defineProps<{ type: Exclude<InstructionType, 'BlockHeader' | 'CallBlock'> }>();

// If/IfElse are wrap/C-blocks, rendered separately below (mirrors
// InstructionRow.vue's own real-canvas split) — a flattened single-line
// `.instruction-content` can't show a bracket shape.
const FIELD_COMPONENTS: Record<Exclude<InstructionDto['type'], 'BlockHeader' | 'CallBlock' | 'If' | 'IfElse'>, Component> = {
  WhenRan: PaletteWhenRanFields,
  Wait: PaletteWaitFields,
  Text: PaletteTextFields,
  Key: PaletteKeyFields,
  Button: PaletteButtonFields,
  MoveMouse: PaletteMoveMouseFields,
  Scroll: PaletteScrollFields,
  Command: PaletteCommandFields,
  Comment: PaletteCommentFields,
  SetVariable: PaletteSetVariableFields,
  ChangeVariable: PaletteChangeVariableFields,
  Return: PaletteReturnFields,
};

const instruction = computed(() => paletteInstructions[props.type]);
const isWrap = computed(() => isWrapType(props.type));
const fieldComponent = computed(() =>
  isWrap.value ? null : FIELD_COMPONENTS[props.type as Exclude<InstructionType, 'BlockHeader' | 'CallBlock' | 'If' | 'IfElse'>],
);
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
  <div v-if="isWrap" class="instruction-row instruction-row-wrap palette-prefab" @pointerdown="onPointerDown">
    <div class="wrap-head-line">
      <component :is="typeIcon" class="instruction-type-icon-inline" />
      <PaletteIfFields v-if="type === 'If'" :instruction="(instruction as Extract<InstructionDto, { type: 'If' }>)" part="head" />
      <PaletteIfElseFields v-else-if="type === 'IfElse'" :instruction="(instruction as Extract<InstructionDto, { type: 'IfElse' }>)" part="head" />
    </div>
    <PaletteIfFields v-if="type === 'If'" :instruction="(instruction as Extract<InstructionDto, { type: 'If' }>)" part="body" />
    <PaletteIfElseFields v-else-if="type === 'IfElse'" :instruction="(instruction as Extract<InstructionDto, { type: 'IfElse' }>)" part="body" />
    <div class="wrap-foot-bar" />
  </div>
  <div
    v-else
    class="instruction-row palette-prefab"
    :class="{ 'instruction-row-when-ran': type === 'WhenRan', 'instruction-row-header': isHeaderType(type), 'instruction-row-cap': isCapType(type) }"
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
