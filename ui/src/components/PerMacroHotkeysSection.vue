<script setup lang="ts">
import { computed } from 'vue';
import { Keyboard } from 'lucide-vue-next';
import { state } from '../store';
import { addMacroHotkey, removeHotkeyBinding, setPendingMacroIdx, startPendingComboCapture } from '../tauri';
import AppDropdown from './AppDropdown.vue';
import AppButton from './AppButton.vue';

const perMacroBindings = computed(() => state.hotkey_bindings.filter(b => b.action.type === 'RunSpecificMacro'));

const macroOptions = computed(() =>
  state.macro_names.map((name, idx) => ({ value: String(idx), label: name })),
);
const pendingMacroValue = computed(() =>
  state.pending_macro_hotkey?.macro_index != null ? String(state.pending_macro_hotkey.macro_index) : '',
);

function onPendingMacroChange(value: string) {
  setPendingMacroIdx(value === '' ? null : parseInt(value, 10));
}

const isCapturingPending = computed(() => state.combo_capture?.kind === 'Pending');
const pendingCombo = computed(() => state.pending_macro_hotkey?.combo_display ?? null);
const canAdd = computed(() => state.pending_macro_hotkey?.macro_index != null && pendingCombo.value != null);
</script>

<template>
  <div class="settings-section">
    <div class="settings-section-title"><Keyboard /><span>Per-Macro Hotkeys</span></div>
    <div>
      <div v-for="b in perMacroBindings" :key="b.binding_index" class="settings-row">
        <span class="settings-row-label">{{ b.macro_name ?? '(deleted)' }}</span>
        <button class="btn-chip">{{ b.combo_display }}</button>
        <AppButton
          class="btn-icon btn-danger"
          icon="x"
          title="Remove hotkey"
          aria-label="Remove hotkey"
          @click="removeHotkeyBinding(b.binding_index)"
        />
      </div>
      <div class="settings-row" id="add-hotkey-form">
        <div class="settings-row-label">Add hotkey:</div>
        <AppDropdown
          :options="macroOptions"
          :model-value="pendingMacroValue"
          placeholder="Select macro…"
          aria-label="Select macro for hotkey"
          @update:model-value="onPendingMacroChange"
        />
        <button
          class="btn-chip"
          :class="{ capturing: isCapturingPending }"
          @click="isCapturingPending ? undefined : startPendingComboCapture()"
        >{{ isCapturingPending ? 'Press combo…' : (pendingCombo ?? 'Set combo') }}</button>
        <AppButton icon="plus" label="Add" :disabled="!canAdd" @click="addMacroHotkey()" />
      </div>
    </div>
  </div>
</template>
