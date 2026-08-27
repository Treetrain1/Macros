<script setup lang="ts">
import { computed } from 'vue';
import { closeSettings } from '../tauri';
import { useTheme } from 'blockstitch';
import { AppButton } from 'blockstitch';

const { currentTheme, toggleTheme } = useTheme();

const themeIcon = computed(() => (currentTheme.value === 'light' ? 'moon' : 'sun'));
const themeLabel = computed(() => (currentTheme.value === 'light' ? 'Switch to dark theme' : 'Switch to light theme'));
</script>

<template>
  <div class="settings-header">
    <AppButton icon="arrow-left" label="Back" @click="closeSettings()" />
    <span style="font-weight:600;">Settings</span>
    <AppButton
      class="btn-icon"
      style="margin-left:auto;"
      :icon="themeIcon"
      :title="themeLabel"
      :aria-label="themeLabel"
      @click="toggleTheme()"
    />
  </div>
</template>
