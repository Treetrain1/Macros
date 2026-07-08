<script setup lang="ts">
import { computed } from 'vue';
import { state } from '../store';
import { clearInstructions, redo, saveMacro, undo } from '../tauri';
import AppButton from './AppButton.vue';

const clearIcon = computed(() => (state.confirm_clear_instructions ? 'alert-triangle' : 'trash'));
const clearLabel = computed(() =>
  state.confirm_clear_instructions
    ? `Confirm clear (${state.confirm_clear_instructions_remaining_secs}s)?`
    : 'Clear instructions',
);
</script>

<template>
  <div class="editor-toolbar">
    <AppButton id="undo-btn" :disabled="!state.can_undo" title="Undo last instruction change" icon="corner-up-left" label="Undo" @click="undo()" />
    <AppButton id="redo-btn" :disabled="!state.can_redo" title="Redo last undone change" icon="corner-up-right" label="Redo" @click="redo()" />
    <AppButton
      id="clear-instructions-btn"
      class="btn-danger"
      :class="{ 'confirm-armed': state.confirm_clear_instructions }"
      title="Remove all instructions"
      :icon="clearIcon"
      :label="clearLabel"
      @click="clearInstructions()"
    />
    <AppButton id="save-macro-btn" class="btn-primary" title="Save macro to disk" icon="save" label="Save macro" @click="saveMacro()" />
  </div>
</template>
