<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { state } from '../store';
import { contextMenu, closeContextMenu } from '../contextMenu';
import { copyAll, copyBlock, clipboardContents, hasClipboard } from '../clipboard';
import { addInstruction, clearInstructions, deleteBlock, deleteInstruction, deleteVariable, pasteInstructions, setRecordingTarget } from '../tauri';
import { clientToCanvas } from '../canvasDrag';
import { ICONS } from '../icons';
import { openRenameVariableDialog } from '../variableDialogs';
import { openEditBlockDialog } from '../blockDialogs';
import { findBlockDef, nextSiblingPath, resolveInstructionAt } from '../types';

const panelRef = ref<HTMLDivElement | null>(null);
const panelStyle = ref<{ left: string; top: string }>({ left: '0px', top: '0px' });

const strand = computed(() => state.current_macro?.strands.find(s => s.id === contextMenu.strandId) ?? null);
const instruction = computed(() => resolveInstructionAt(strand.value, contextMenu.path));
const headerBlockId = computed(() => (instruction.value?.type === 'BlockHeader' ? instruction.value.block_id : null));
const isRecordingTarget = computed(
  () => contextMenu.strandId !== '' && contextMenu.strandId === state.current_macro?.recording_target_strand_id,
);

const totalBlocks = computed(() =>
  (state.current_macro?.strands ?? []).reduce((n, s) => n + s.instructions.length, 0),
);
const clearIcon = computed(() => (state.confirm_clear_instructions ? 'alert-triangle' : 'trash'));
const clearLabel = computed(() =>
  state.confirm_clear_instructions
    ? `Confirm delete (${state.confirm_clear_instructions_remaining_secs}s)?`
    : `Delete ${totalBlocks.value} Blocks`,
);

function positionPanel() {
  const panel = panelRef.value;
  if (!panel) return;
  const left = Math.max(4, Math.min(contextMenu.x, window.innerWidth - panel.offsetWidth - 4));
  const top = Math.max(4, Math.min(contextMenu.y, window.innerHeight - panel.offsetHeight - 4));
  panelStyle.value = { left: `${left}px`, top: `${top}px` };
}

function onOutsideMouseDown(e: MouseEvent) {
  if (panelRef.value?.contains(e.target as Node)) return;
  closeContextMenu();
}
function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') closeContextMenu();
}
function onScrollOrResize() {
  closeContextMenu();
}

watch(
  () => contextMenu.open,
  async open => {
    if (open) {
      await nextTick();
      positionPanel();
      document.addEventListener('mousedown', onOutsideMouseDown, true);
      document.addEventListener('keydown', onKeydown);
      window.addEventListener('scroll', onScrollOrResize, true);
      window.addEventListener('resize', onScrollOrResize);
    } else {
      document.removeEventListener('mousedown', onOutsideMouseDown, true);
      document.removeEventListener('keydown', onKeydown);
      window.removeEventListener('scroll', onScrollOrResize, true);
      window.removeEventListener('resize', onScrollOrResize);
    }
  },
);

onBeforeUnmount(() => closeContextMenu());

function deleteBlockPosition(): [number, number] {
  return clientToCanvas(contextMenu.x, contextMenu.y);
}

function onSetRecordingTarget() {
  setRecordingTarget(contextMenu.strandId);
  closeContextMenu();
}
function onDeleteBlock() {
  // A custom block's header row: delete the block definition (which also
  // removes this body strand and every call site referencing it) rather
  // than just detaching the strand and leaving an orphaned "My Blocks"
  // prefab behind — mirrors the drag-to-trash handling in canvasDrag.ts.
  if (headerBlockId.value) {
    deleteBlock(headerBlockId.value);
    closeContextMenu();
    return;
  }
  const [x, y] = deleteBlockPosition();
  deleteInstruction(contextMenu.strandId, contextMenu.path, x, y);
  closeContextMenu();
}
function onCopyBlock() {
  if (strand.value) copyBlock(strand.value, contextMenu.path);
  closeContextMenu();
}
function onCopyAll() {
  if (strand.value) copyAll(strand.value, contextMenu.path);
  closeContextMenu();
}
function onCutBlock() {
  if (strand.value) copyBlock(strand.value, contextMenu.path);
  const [x, y] = deleteBlockPosition();
  deleteInstruction(contextMenu.strandId, contextMenu.path, x, y);
  closeContextMenu();
}
function onDuplicateBlock() {
  const ins = instruction.value;
  if (ins) addInstruction(contextMenu.strandId, nextSiblingPath(contextMenu.path), { ...ins });
  closeContextMenu();
}
function onDeleteAllBlocks() {
  // Deliberately doesn't close the menu — the two-click confirm (armed on
  // first click, executed on second, matching the toolbar button this
  // replaced) needs the menu to stay open so the label/icon can update in
  // between; it closes via the normal outside-click/Escape handling once
  // the user's done with it.
  clearInstructions();
}
function onPaste() {
  const contents = clipboardContents();
  if (contents) pasteInstructions(contextMenu.canvasX, contextMenu.canvasY, contents);
  closeContextMenu();
}
function onRenameVariable() {
  openRenameVariableDialog(contextMenu.variableName);
  closeContextMenu();
}
function onDeleteVariable() {
  deleteVariable(contextMenu.variableName);
  closeContextMenu();
}
function onEditBlock() {
  const def = findBlockDef(state.current_macro, contextMenu.blockId);
  if (def) openEditBlockDialog(def);
  closeContextMenu();
}
function onEditBlockHeader() {
  const def = headerBlockId.value ? findBlockDef(state.current_macro, headerBlockId.value) : undefined;
  if (def) openEditBlockDialog(def);
  closeContextMenu();
}
function onDeleteBlockDef() {
  deleteBlock(contextMenu.blockId);
  closeContextMenu();
}
</script>

<template>
  <Teleport to="#dd-portal">
    <div v-if="contextMenu.open" ref="panelRef" class="context-menu" role="menu" :style="panelStyle">
      <template v-if="contextMenu.type === 'block'">
        <button v-if="headerBlockId" type="button" class="context-menu-item" role="menuitem" @click="onEditBlockHeader">
          <span class="context-menu-item-icon"><component :is="ICONS.blocks" /></span>
          <span>Edit block</span>
        </button>
        <button type="button" class="context-menu-item" :class="{ 'context-menu-item-active': isRecordingTarget }" role="menuitem" @click="onSetRecordingTarget">
          <span class="context-menu-item-icon"><component :is="ICONS.target" /></span>
          <span>{{ isRecordingTarget ? 'Recording Target (current)' : 'Set Recording Target' }}</span>
        </button>
        <button type="button" class="context-menu-item" role="menuitem" @click="onCopyBlock">
          <span class="context-menu-item-icon"><component :is="ICONS.copy" /></span>
          <span>Copy block</span>
        </button>
        <button type="button" class="context-menu-item" role="menuitem" @click="onCopyAll">
          <span class="context-menu-item-icon"><component :is="ICONS.layers" /></span>
          <span>Copy all</span>
        </button>
        <button type="button" class="context-menu-item" role="menuitem" @click="onDuplicateBlock">
          <span class="context-menu-item-icon"><component :is="ICONS['corner-down-right']" /></span>
          <span>Duplicate block</span>
        </button>
        <button type="button" class="context-menu-item" role="menuitem" @click="onCutBlock">
          <span class="context-menu-item-icon"><component :is="ICONS.scissors" /></span>
          <span>Cut block</span>
        </button>
        <button type="button" class="context-menu-item context-menu-item-danger" role="menuitem" @click="onDeleteBlock">
          <span class="context-menu-item-icon"><component :is="ICONS.trash" /></span>
          <span>Delete block</span>
        </button>
      </template>
      <template v-else-if="contextMenu.type === 'canvas'">
        <button
          type="button"
          class="context-menu-item context-menu-item-danger"
          :class="{ 'confirm-armed': state.confirm_clear_instructions }"
          role="menuitem"
          @click="onDeleteAllBlocks"
        >
          <span class="context-menu-item-icon"><component :is="ICONS[clearIcon]" /></span>
          <span>{{ clearLabel }}</span>
        </button>
        <button type="button" class="context-menu-item" role="menuitem" :disabled="!hasClipboard()" @click="onPaste">
          <span class="context-menu-item-icon"><component :is="ICONS['clipboard-paste']" /></span>
          <span>Paste</span>
        </button>
      </template>
      <template v-else-if="contextMenu.type === 'variable'">
        <button type="button" class="context-menu-item" role="menuitem" @click="onRenameVariable">
          <span class="context-menu-item-icon"><component :is="ICONS.equal" /></span>
          <span>Rename variable</span>
        </button>
        <button type="button" class="context-menu-item context-menu-item-danger" role="menuitem" @click="onDeleteVariable">
          <span class="context-menu-item-icon"><component :is="ICONS.trash" /></span>
          <span>Delete variable</span>
        </button>
      </template>
      <template v-else>
        <button type="button" class="context-menu-item" role="menuitem" @click="onEditBlock">
          <span class="context-menu-item-icon"><component :is="ICONS.blocks" /></span>
          <span>Edit block</span>
        </button>
        <button type="button" class="context-menu-item context-menu-item-danger" role="menuitem" @click="onDeleteBlockDef">
          <span class="context-menu-item-icon"><component :is="ICONS.trash" /></span>
          <span>Delete block</span>
        </button>
      </template>
    </div>
  </Teleport>
</template>
