<script setup lang="ts">
import { Trash2 } from 'lucide-vue-next';
import { INSTRUCTION_TYPE_LABELS } from '../icons';
import PaletteInstructionBlock from './PaletteInstructionBlock.vue';
import PaletteValueBlock from './PaletteValueBlock.vue';
import { beginSidebarResize, sidebarWidth } from '../composables/useSidebarWidth';
import type { InstructionDto, ValueKind } from '../types';

const instructionTypes = Object.keys(INSTRUCTION_TYPE_LABELS) as InstructionDto['type'][];

const VALUE_KINDS: ValueKind[] = ['Number', 'Text', 'Add', 'Sub', 'Mul', 'Div'];
</script>

<template>
  <div class="instruction-sidebar" id="instruction-sidebar" :style="{ width: sidebarWidth + 'px' }">
    <div class="sidebar-trash-hint">
      <Trash2 />
      <span>Drag a block here to delete it</span>
    </div>
    <div class="sidebar-scroll">
      <div class="sidebar-palette" id="sidebar-palette">
        <PaletteInstructionBlock v-for="type in instructionTypes" :key="type" :type="type" />
      </div>

      <div class="sidebar-section-label">Operator</div>
      <div class="sidebar-palette sidebar-palette-values" id="sidebar-palette-values">
        <PaletteValueBlock v-for="kind in VALUE_KINDS" :key="kind" :kind="kind" />
      </div>
    </div>
    <div class="sidebar-resize-handle" @pointerdown="beginSidebarResize" />
  </div>
</template>
