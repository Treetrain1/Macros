<script setup lang="ts">
import { ref } from 'vue';
import { AppWindow } from 'lucide-vue-next';
import AppSelectorDialog from '../../AppSelectorDialog.vue';
import type { AppEntryDto, InstructionDto } from '../../../types';

const props = defineProps<{ instruction: Extract<InstructionDto, { type: 'OpenApp' }> }>();

const showDialog = ref(false);

function onSelect(app: AppEntryDto) {
  props.instruction.command = app.command;
  props.instruction.name = app.name;
  props.instruction.icon = app.icon;
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
