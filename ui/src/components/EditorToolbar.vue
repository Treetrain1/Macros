<script setup lang="ts">
import { state } from '../store';
import { redo, saveMacro, undo } from '../tauri';
import { AppButton } from 'blockstitch';
</script>

<template>
  <div class="editor-toolbar">
    <AppButton id="undo-btn" :disabled="!state.can_undo" title="Undo last instruction change" icon="corner-up-left" label="Undo" @click="undo()" />
    <AppButton id="redo-btn" :disabled="!state.can_redo" title="Redo last undone change" icon="corner-up-right" label="Redo" @click="redo()" />
    <AppButton id="save-macro-btn" class="btn-primary" title="Save macro to disk" icon="save" label="Save macro" @click="saveMacro()" />
  </div>
</template>
