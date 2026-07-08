<script setup lang="ts">
import { computed } from 'vue';
import { state } from '../store';
import { newMacro, openSettings, removeMacro, selectMacro } from '../tauri';
import AppDropdown from './AppDropdown.vue';
import AppButton from './AppButton.vue';

const macroOptions = computed(() =>
  state.macro_names.map((name, idx) => ({ value: String(idx), label: name })),
);
const selectedValue = computed(() => (state.macro_selected != null ? String(state.macro_selected) : ''));

function onSelect(value: string) {
  if (value === '') return;
  const idx = parseInt(value, 10);
  const cached = state.macros_data[idx];
  if (cached) {
    state.macro_selected = idx;
    state.current_macro = cached;
    state.can_undo = false;
    state.can_redo = false;
    state.invalid_field_buffers = [];
    state.key_capture = null;
  }
  selectMacro(idx);
}

const removeIcon = computed(() => (state.confirm_remove_macro ? 'alert-triangle' : 'trash'));
const removeLabel = computed(() =>
  state.confirm_remove_macro ? `Delete (${state.confirm_remove_macro_remaining_secs}s)?` : 'Delete',
);
</script>

<template>
  <div class="toolbar-row" id="selector-row">
    <div class="selector-left">
      <label id="macro-dropdown-label">Select macro</label>
      <AppDropdown
        :options="macroOptions"
        :model-value="selectedValue"
        placeholder="— no macro selected —"
        class-name="macro-select-trigger"
        aria-label="Select macro"
        aria-labelledby="macro-dropdown-label"
        @update:model-value="onSelect"
      />
    </div>
    <div class="selector-right">
      <AppButton icon="plus" label="New macro" title="Add a new macro" @click="newMacro()" />
      <AppButton
        id="remove-macro-btn"
        class="btn-danger"
        :class="{ 'confirm-armed': state.confirm_remove_macro }"
        :disabled="state.macro_selected == null"
        title="Delete selected macro"
        :icon="removeIcon"
        :label="removeLabel"
        @click="removeMacro()"
      />
      <AppButton icon="settings" label="Settings" title="Open Settings" @click="openSettings()" />
    </div>
  </div>
</template>
