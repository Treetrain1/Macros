<script setup lang="ts">
import { computed } from 'vue';
import { Trash2 } from 'lucide-vue-next';
import { INSTRUCTION_TYPE_LABELS } from '../icons';
import PaletteInstructionBlock from './PaletteInstructionBlock.vue';
import PaletteValueBlock from './PaletteValueBlock.vue';
import PaletteCallBlock from './PaletteCallBlock.vue';
import PaletteCallValueBlock from './PaletteCallValueBlock.vue';
import MakeVariableDialog from './MakeVariableDialog.vue';
import MakeBlockDialog from './MakeBlockDialog.vue';
import { beginSidebarResize, sidebarWidth } from '../composables/useSidebarWidth';
import { OPERATOR_KINDS } from '../valueOps';
import { state } from '../store';
import { sortedVariableNames } from '../types';
import type { InstructionDto, ValueKind } from '../types';
import { closeVariableDialog, openCreateVariableDialog, variableDialog } from '../variableDialogs';
import { blockDialog, closeBlockDialog, openCreateBlockDialog } from '../blockDialogs';

// SetVariable/ChangeVariable render in the Variables section below and
// Return in the "My Blocks" section, not here; BlockHeader/CallBlock are
// never dragged from a fixed prefab at all (see PaletteInstructionBlock.vue);
// Comment is a floating note now (right-click canvas/a block), not a sidebar
// prefab — all filtered out of the generic "Instruction" group.
const instructionTypes = (Object.keys(INSTRUCTION_TYPE_LABELS) as InstructionDto['type'][])
  .filter((t): t is Exclude<InstructionDto['type'], 'SetVariable' | 'ChangeVariable' | 'BlockHeader' | 'CallBlock' | 'Return' | 'Comment'> =>
    t !== 'SetVariable' && t !== 'ChangeVariable' && t !== 'BlockHeader' && t !== 'CallBlock' && t !== 'Return' && t !== 'Comment');

const commandBlocks = computed(() => (state.current_macro?.block_defs ?? []).filter(b => !b.returns_value));
const reporterBlocks = computed(() => (state.current_macro?.block_defs ?? []).filter(b => b.returns_value));

// Number/Text literals, plus every operator registered in valueOps.ts's
// OPERATOR_KINDS — adding an operator there is enough to get it a palette
// entry, no edit needed here.
const VALUE_KINDS: ValueKind[] = ['Number', 'Text', ...OPERATOR_KINDS.map(s => s.kind)];

// One reporter block per declared variable, alphabetical.
const variableKinds = computed<ValueKind[]>(() => sortedVariableNames(state.current_macro).map(n => `Var:${n}` as ValueKind));
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

      <div class="sidebar-section-label-row">
        <span class="sidebar-section-label">Variables</span>
        <button type="button" class="btn-make-variable" @click="openCreateVariableDialog">Make a Variable</button>
      </div>
      <div class="sidebar-palette sidebar-palette-values" id="sidebar-palette-variables">
        <PaletteValueBlock v-for="kind in variableKinds" :key="kind" :kind="kind" />
      </div>
      <div class="sidebar-palette">
        <PaletteInstructionBlock type="SetVariable" />
        <PaletteInstructionBlock type="ChangeVariable" />
      </div>

      <div class="sidebar-section-label-row">
        <span class="sidebar-section-label">My Blocks</span>
        <button type="button" class="btn-make-variable" @click="openCreateBlockDialog">Make a Block</button>
      </div>
      <div class="sidebar-palette sidebar-palette-values" v-if="reporterBlocks.length">
        <PaletteCallValueBlock v-for="def in reporterBlocks" :key="def.id" :def="def" />
      </div>
      <div class="sidebar-palette">
        <PaletteCallBlock v-for="def in commandBlocks" :key="def.id" :def="def" />
        <PaletteInstructionBlock type="Return" />
      </div>
    </div>
    <div class="sidebar-resize-handle" @pointerdown="beginSidebarResize" />
  </div>
  <MakeVariableDialog
    v-if="variableDialog.mode"
    :rename-target="variableDialog.mode === 'rename' ? variableDialog.renameTarget : null"
    @close="closeVariableDialog"
  />
  <MakeBlockDialog
    v-if="blockDialog.mode"
    :edit-target="blockDialog.mode === 'edit' ? blockDialog.editTarget : null"
    @close="closeBlockDialog"
  />
</template>
