<script setup lang="ts">
import { computed } from 'vue';
import { RefreshCw } from 'lucide-vue-next';
import { appVersion, state } from '../store';
import { applyUpdate, checkForUpdates } from '../tauri';

const uc = computed(() => state.update_check_state);
const busy = computed(() => uc.value.state === 'Checking' || uc.value.state === 'Applying');

const statusMessage = computed(() => {
  switch (uc.value.state) {
    case 'Checking': return 'Checking for updates…';
    case 'UpToDate': return 'Up to date';
    case 'UpdateAvailable': return `Update available: ${uc.value.version}`;
    case 'Applying': return 'Installing update…';
    case 'Error': return `Update check failed: ${uc.value.error}`;
    default: return '';
  }
});
</script>

<template>
  <div class="settings-section">
    <div class="settings-section-title"><RefreshCw /><span>Updates</span></div>
    <div>
      <div class="settings-row">
        <span class="settings-row-label">{{ appVersion ? `Current version: ${appVersion}` : 'Updates' }}</span>
        <button :disabled="busy" @click="checkForUpdates()">Check for Updates</button>
      </div>
      <div v-if="uc.state !== 'Idle'" class="settings-row">{{ statusMessage }}</div>
      <div v-if="uc.state === 'UpdateAvailable'" class="settings-row">
        <button @click="applyUpdate()">Update Now</button>
      </div>
    </div>
  </div>
</template>
