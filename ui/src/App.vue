<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue';
import { state, initState } from './store';
import { cancelComboCapture, comboCaptureEvent, keyCaptureEvent } from './tauri';
import MainPage from './components/MainPage.vue';
import SettingsPage from './components/SettingsPage.vue';

onMounted(() => {
  initState();
  document.addEventListener('keydown', onKeydown);
});
onUnmounted(() => {
  document.removeEventListener('keydown', onKeydown);
});

async function onKeydown(e: KeyboardEvent) {
  if (state.key_capture != null) {
    e.preventDefault();
    await keyCaptureEvent(e.code, e.key);
    return;
  }
  if (state.combo_capture != null) {
    e.preventDefault();
    if (e.key === 'Escape') {
      await cancelComboCapture();
      return;
    }
    if (['Control', 'Shift', 'Alt', 'Meta'].includes(e.key)) return;
    const modifiers = (e.ctrlKey ? 1 : 0) | (e.shiftKey ? 2 : 0) | (e.altKey ? 4 : 0) | (e.metaKey ? 8 : 0);
    await comboCaptureEvent(e.code, modifiers);
  }
}
</script>

<template>
  <MainPage :class="{ 'page-hidden': state.page === 'Settings' }" />
  <SettingsPage :class="{ 'page-hidden': state.page !== 'Settings' }" />
</template>
