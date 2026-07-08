<script setup lang="ts">
import { computed } from 'vue';
import { state } from '../store';
import { setTitle } from '../tauri';
import InstructionSidebar from './InstructionSidebar.vue';
import Canvas from './Canvas.vue';
import EditorToolbar from './EditorToolbar.vue';

const isRecording = computed(() => state.recording_phase.phase === 'Active');

function onTitleInput(e: Event) {
  setTitle((e.target as HTMLInputElement).value);
}
</script>

<template>
  <div id="macro-editor" :class="{ recording: isRecording }">
    <div class="editor-title-row">
      <label for="macro-title">Title</label>
      <input type="text" id="macro-title" placeholder="Macro name" :value="state.current_macro?.name" @input="onTitleInput">
    </div>

    <div class="editor-content-area">
      <InstructionSidebar />
      <Canvas :recording="isRecording" />
    </div>

    <EditorToolbar />
  </div>
</template>
