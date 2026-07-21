// Reactive singleton coordinating the variable name popup (MakeVariableDialog.vue)
// between its two triggers — the sidebar's "Make a Variable" button and the
// per-variable context menu's "Rename variable" — which live in different
// components (InstructionSidebar.vue and ContextMenu.vue respectively).
import { reactive } from 'vue';

type VariableDialogMode = 'create' | 'rename' | null;

interface VariableDialogState {
  mode: VariableDialogMode;
  renameTarget: string;
}

export const variableDialog = reactive<VariableDialogState>({ mode: null, renameTarget: '' });

export function openCreateVariableDialog(): void {
  variableDialog.mode = 'create';
  variableDialog.renameTarget = '';
}

export function openRenameVariableDialog(name: string): void {
  variableDialog.mode = 'rename';
  variableDialog.renameTarget = name;
}

export function closeVariableDialog(): void {
  variableDialog.mode = null;
}
