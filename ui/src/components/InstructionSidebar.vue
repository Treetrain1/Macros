<script setup lang="ts">
import { Trash2 } from 'lucide-vue-next';
import { ICONS } from '../icons';
import { INSTRUCTION_TYPE_ICONS, INSTRUCTION_TYPE_LABELS } from '../icons';
import { beginPaletteDrag, defaultInstruction } from '../canvasDrag';
import InstructionRow from './InstructionRow.vue';
import type { InstructionDto } from '../types';

const instructionTypes = Object.keys(INSTRUCTION_TYPE_LABELS) as InstructionDto['type'][];

// Hidden real-InstructionRow templates (one per instruction type), used only
// as a clone source when a palette drag starts — this way the drag ghost is
// pixel-identical to a real instruction block instead of a hand-rebuilt copy.
const ghostEls = new Map<InstructionDto['type'], HTMLElement>();
function registerGhost(type: InstructionDto['type'], componentPublicInstance: unknown) {
  const el = (componentPublicInstance as { $el?: HTMLElement } | null)?.$el;
  if (el) ghostEls.set(type, el);
}

function onPaletteDown(e: PointerEvent, type: InstructionDto['type']) {
  const src = ghostEls.get(type);
  if (!src) return;
  beginPaletteDrag(e, type, src.cloneNode(true) as HTMLElement);
}
</script>

<template>
  <div class="instruction-sidebar" id="instruction-sidebar">
    <div class="sidebar-trash-hint">
      <Trash2 />
      <span>Drag a strand here to delete it</span>
    </div>
    <div class="sidebar-palette" id="sidebar-palette">
      <div
        v-for="type in instructionTypes"
        :key="type"
        class="palette-block"
        @pointerdown="onPaletteDown($event, type)"
      >
        <component :is="ICONS[INSTRUCTION_TYPE_ICONS[type]]" />
        <span>{{ INSTRUCTION_TYPE_LABELS[type] }}</span>
      </div>
    </div>

    <div style="position: absolute; visibility: hidden; pointer-events: none; left: -9999px; top: -9999px;">
      <InstructionRow
        v-for="type in instructionTypes"
        :key="`ghost-${type}`"
        :ref="(el) => registerGhost(type, el)"
        strand-id="__palette__"
        :index="0"
        :instruction="defaultInstruction(type)"
      />
    </div>
  </div>
</template>
