<script setup lang="ts">
import { ref } from 'vue';
import { AppWindow } from 'lucide-vue-next';
import { editInstruction } from '../../tauri';
import AppSelectorDialog from '../AppSelectorDialog.vue';
import type { AppEntryDto, InstrPath, InstructionDto } from '../../types';

const props = defineProps<{ strandId: string; path: InstrPath; instruction: Extract<InstructionDto, { type: 'OpenApp' }> }>();

const showDialog = ref(false);

function onSelect(app: AppEntryDto) {
  editInstruction(props.strandId, props.path, { type: 'OpenApp', command: app.command, name: app.name, icon: app.icon });
  showDialog.value = false;
}
</script>

<template>
  <span class="instruction-label">Open:</span>
  <button type="button" class="btn-chip open-app-choose-btn" @click="showDialog = true">
    <img v-if="instruction.icon" :src="instruction.icon" class="open-app-icon" alt="">
    <AppWindow v-else-if="instruction.name" :size="14" class="open-app-icon-fallback" />
    <span v-if="instruction.name" class="open-app-name">{{ instruction.name }}</span>
    <span v-else>Choose App…</span>
  </button>
  <AppSelectorDialog v-if="showDialog" @select="onSelect" @close="showDialog = false" />
</template>
