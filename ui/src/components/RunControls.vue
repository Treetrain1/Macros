<script setup lang="ts">
import { computed } from 'vue';
import { state } from '../store';
import { runMacro, startRecording, stopRecording, toggleLoopMode } from '../tauri';
import AppButton from './AppButton.vue';
import SwitchControl from './SwitchControl.vue';

const runIcon = computed(() => (state.loop_mode_enabled ? 'repeat' : 'play'));
const runLabel = computed(() => (state.loop_mode_enabled ? 'Start loop' : 'Run macro'));

const recordClass = computed(() => {
  const phase = state.recording_phase.phase;
  if (phase === 'Countdown') return 'btn-record btn-record-countdown';
  if (phase === 'Active') return 'btn-active-record';
  return 'btn-record';
});
const recordIcon = computed(() => {
  const phase = state.recording_phase.phase;
  if (phase === 'Countdown') return 'pause';
  if (phase === 'Active') return 'square';
  return 'circle';
});
const stopRecordingCombo = computed(() =>
  state.hotkey_bindings.find(b => b.action.type === 'StopRecording')?.combo_display
);
const recordLabel = computed(() => {
  const phase = state.recording_phase;
  if (phase.phase === 'Countdown') return `Recording in ${phase.countdown}s…`;
  if (phase.phase === 'Active') {
    return stopRecordingCombo.value ? `Stop recording (${stopRecordingCombo.value})` : 'Stop recording';
  }
  return 'Record';
});
const recordDisabled = computed(() => state.recording_phase.phase === 'Idle' && state.macro_selected == null);

function onRecordClick() {
  if (state.recording_phase.phase === 'Idle') startRecording();
  else stopRecording();
}

</script>

<template>
  <div class="toolbar-row" id="run-row">
    <div class="run-left">
      <AppButton
        id="run-macro-btn"
        class="btn-primary"
        :disabled="state.macro_selected == null"
        title="Runs the current macro once or starts looping if enabled"
        :icon="runIcon"
        :label="runLabel"
        @click="runMacro()"
      />
      <SwitchControl :model-value="state.loop_mode_enabled" @update:model-value="toggleLoopMode">
        Loop mode
      </SwitchControl>
    </div>
    <div class="run-right">
      <AppButton
        id="record-btn"
        :class="recordClass"
        :disabled="recordDisabled"
        title="Records keyboard and mouse events into the macro"
        :icon="recordIcon"
        :label="recordLabel"
        @click="onRecordClick"
      />
    </div>
  </div>
</template>
