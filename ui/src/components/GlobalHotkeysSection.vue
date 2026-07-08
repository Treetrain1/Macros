<script setup lang="ts">
import { KeyRound } from 'lucide-vue-next';
import { state } from '../store';
import { clearNamedHotkey, resetHotkeyToDefault, startComboCapture } from '../tauri';
import { NAMED_ACTIONS, type NamedActionType } from '../constants';
import AppButton from './AppButton.vue';

function bindingFor(type: NamedActionType) {
  return state.hotkey_bindings.find(b => b.action.type === type);
}
function isCapturing(type: NamedActionType) {
  return state.combo_capture?.kind === 'Named' && state.combo_capture.action?.type === type;
}
</script>

<template>
  <div class="settings-section">
    <div class="settings-section-title"><KeyRound /><span>Global Hotkeys</span></div>
    <div>
      <div v-for="{ label, type } in NAMED_ACTIONS" :key="type" class="settings-row">
        <span class="settings-row-label">{{ label }}</span>
        <button
          class="btn-chip"
          :class="{ capturing: isCapturing(type) }"
          :disabled="false"
          @click="isCapturing(type) ? undefined : startComboCapture({ type })"
        >
          {{ isCapturing(type) ? 'Press combo…' : (bindingFor(type)?.combo_display ?? 'Not set') }}
        </button>
        <button
          v-show="!isCapturing(type) && bindingFor(type)?.combo_display != null"
          @click="resetHotkeyToDefault({ type })"
        >Default</button>
        <AppButton
          v-show="!isCapturing(type) && bindingFor(type)?.combo_display != null"
          class="btn-icon btn-danger"
          icon="x"
          title="Clear hotkey"
          aria-label="Clear hotkey"
          @click="clearNamedHotkey({ type })"
        />
      </div>
    </div>
  </div>
</template>
