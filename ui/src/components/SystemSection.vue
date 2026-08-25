<script setup lang="ts">
import { Minimize2 } from 'lucide-vue-next';
import { state } from '../store';
import { setCloseToTray } from '../tauri';
import SwitchControl from './SwitchControl.vue';
</script>

<template>
  <div class="settings-section">
    <div class="settings-section-title"><Minimize2 /><span>System</span></div>
    <div>
      <div class="settings-row">
        <span class="settings-row-label">Keep running in the system tray when closed</span>
        <SwitchControl :model-value="state.close_to_tray" @update:model-value="setCloseToTray" />
      </div>
    </div>
  </div>
</template>
