// Reactive singleton coordinating the "Make a Block"/"Edit Block" popup
// (MakeBlockDialog.vue) between its two triggers — the "My Blocks" section's
// "Make a Block" button (InstructionSidebar.vue) and a prefab's "Edit Block"
// context-menu item (ContextMenu.vue) — same shape as variableDialogs.ts.
import { reactive } from 'vue';
import type { BlockDefDto } from './types';

type BlockDialogMode = 'create' | 'edit' | null;

interface BlockDialogState {
  mode: BlockDialogMode;
  // Snapshot of the block being edited, taken at open time — the dialog
  // edits its own local copy and only writes back via editBlock() on OK, so
  // it doesn't need to track live changes to the real def while open.
  editTarget: BlockDefDto | null;
}

export const blockDialog = reactive<BlockDialogState>({ mode: null, editTarget: null });

export function openCreateBlockDialog(): void {
  blockDialog.mode = 'create';
  blockDialog.editTarget = null;
}

export function openEditBlockDialog(def: BlockDefDto): void {
  blockDialog.mode = 'edit';
  blockDialog.editTarget = def;
}

export function closeBlockDialog(): void {
  blockDialog.mode = null;
  blockDialog.editTarget = null;
}
