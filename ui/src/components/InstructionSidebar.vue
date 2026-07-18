<script setup lang="ts">
import { Trash2 } from 'lucide-vue-next';
import { ICONS } from '../icons';
import { INSTRUCTION_TYPE_ICONS, INSTRUCTION_TYPE_LABELS } from '../icons';
import { beginPaletteDrag, defaultInstruction } from '../canvasDrag';
import { beginValuePaletteDrag } from '../valueDrag';
import InstructionRow from './InstructionRow.vue';
import ValueBlock from './ValueBlock.vue';
import type { InstructionDto, ValueKind, ValueLocationDto } from '../types';
import { defaultValueForKind } from '../types';

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

const VALUE_KINDS: ValueKind[] = ['Number', 'Text', 'Add', 'Sub', 'Mul', 'Div'];
const VALUE_KIND_LABELS: Record<ValueKind, string> = {
  Number: 'Number',
  Text: 'Text',
  Add: 'Add',
  Sub: 'Subtract',
  Mul: 'Multiply',
  Div: 'Divide',
};

// Never resolved against real state — just gives the hidden ghost templates
// below a well-formed ValueLocationDto to render with.
const GHOST_LOCATION: ValueLocationDto = { kind: 'Field', strand_id: '__palette__', index: 0, field_id: '', path: [] };

// Same idea as ghostEls above, but for value blocks: a hidden ValueBlock per
// kind, cloned (in valueDrag.ts, at actual drag-start) as the drag ghost.
const valueGhostEls = new Map<ValueKind, HTMLElement>();
function registerValueGhost(kind: ValueKind, componentPublicInstance: unknown) {
  const el = (componentPublicInstance as { $el?: HTMLElement } | null)?.$el;
  if (el) valueGhostEls.set(kind, el);
}

function onValuePaletteDown(e: PointerEvent, kind: ValueKind) {
  const src = valueGhostEls.get(kind);
  if (!src) return;
  beginValuePaletteDrag(e, kind, src);
}
</script>

<template>
  <div class="instruction-sidebar" id="instruction-sidebar">
    <div class="sidebar-trash-hint">
      <Trash2 />
      <span>Drag a block here to delete it</span>
    </div>
    <div class="sidebar-scroll">
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

      <div class="sidebar-section-label">Operator</div>
      <div class="sidebar-palette sidebar-palette-values" id="sidebar-palette-values">
        <div
          v-for="kind in VALUE_KINDS"
          :key="kind"
          class="palette-block"
          @pointerdown="onValuePaletteDown($event, kind)"
        >
          <span>{{ VALUE_KIND_LABELS[kind] }}</span>
        </div>
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
      <ValueBlock
        v-for="kind in VALUE_KINDS"
        :key="`value-ghost-${kind}`"
        :ref="(el) => registerValueGhost(kind, el)"
        :location="GHOST_LOCATION"
        :value="defaultValueForKind(kind)"
      />
    </div>
  </div>
</template>
