<script setup lang="ts">
import { computed, ref } from 'vue';
import { Inbox } from 'lucide-vue-next';
import { state } from '../store';
import { exportMacro, importMacro } from '../tauri';
import type { ImportPromptDto } from '../types';
import AppDropdown from './AppDropdown.vue';
import AppButton from './AppButton.vue';
import ImportReviewDialog from './ImportReviewDialog.vue';

const exportMacroId = ref('');
const macroOptions = computed(() =>
  state.macros_data.map(m => ({ value: m.id, label: m.name })),
);

function onExportMacroChange(value: string) {
  exportMacroId.value = value;
}

const exportBusy = ref(false);
const importBusy = ref(false);
const errorMessage = ref<string | null>(null);
const importPrompt = ref<ImportPromptDto | null>(null);

async function onExport() {
  if (!exportMacroId.value || exportBusy.value) return;
  errorMessage.value = null;
  exportBusy.value = true;
  try {
    await exportMacro(exportMacroId.value);
  } catch (e) {
    errorMessage.value = `Failed to export macro: ${e}`;
  } finally {
    exportBusy.value = false;
  }
}

async function onImport() {
  if (importBusy.value) return;
  errorMessage.value = null;
  importBusy.value = true;
  try {
    importPrompt.value = await importMacro();
  } catch (e) {
    errorMessage.value = `Failed to import macro: ${e}`;
  } finally {
    importBusy.value = false;
  }
}
</script>

<template>
  <div class="settings-section">
    <div class="settings-section-title"><Inbox /><span>Import / Export</span></div>
    <div>
      <div class="settings-row">
        <span class="settings-row-label">Export macro:</span>
        <AppDropdown
          :options="macroOptions"
          :model-value="exportMacroId"
          placeholder="Select macro…"
          aria-label="Select macro to export"
          @update:model-value="onExportMacroChange"
        />
        <AppButton
          icon="upload"
          label="Export…"
          :disabled="!exportMacroId || exportBusy"
          @click="onExport"
        />
      </div>
      <div class="settings-row">
        <span class="settings-row-label">Import a .macro file as a new macro</span>
        <AppButton icon="download" label="Import…" :disabled="importBusy" @click="onImport" />
      </div>
      <div v-if="errorMessage" class="settings-row">{{ errorMessage }}</div>
    </div>
    <ImportReviewDialog v-if="importPrompt" :prompt="importPrompt" @close="importPrompt = null" />
  </div>
</template>
