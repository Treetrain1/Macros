<script setup lang="ts">
import { Server } from 'lucide-vue-next';
import { state } from '../store';
import { setIpcAutoStart, setIpcPortText, startIpcServer, stopIpcServer } from '../tauri';
import SwitchControl from './SwitchControl.vue';

function onPortInput(e: Event) {
  setIpcPortText((e.target as HTMLInputElement).value);
}
</script>

<template>
  <div class="settings-section">
    <div class="settings-section-title"><Server /><span>TCP Server</span></div>
    <div>
      <div class="settings-row">
        <span class="settings-row-label">Port</span>
        <input
          type="text"
          style="width: 80px;"
          :value="state.ipc_port_text"
          :class="{ invalid: state.ipc_port_invalid, 'shake-once': state.ipc_port_invalid }"
          @input="onPortInput"
        >
      </div>
      <div class="settings-row">
        <span class="settings-row-label">
          {{ state.ipc_active_port != null ? `Listening on 127.0.0.1:${state.ipc_active_port}` : 'Stopped' }}
        </span>
        <button v-if="state.ipc_active_port != null" @click="stopIpcServer()">Stop Server</button>
        <button v-else :disabled="state.ipc_port_invalid" @click="startIpcServer()">Start Server</button>
      </div>
      <div class="settings-row">
        <span class="settings-row-label">Automatically start server on app launch</span>
        <SwitchControl :model-value="state.ipc_auto_start" @update:model-value="setIpcAutoStart" />
      </div>
    </div>
  </div>
</template>
