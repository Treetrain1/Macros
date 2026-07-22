<script setup lang="ts">
import { computed } from 'vue';
import { state } from '../../store';
import { editInstruction, startKeyCapture } from '../../tauri';
import AppDropdown from '../AppDropdown.vue';
import type { InstructionDto, KeyDirection } from '../../types';

const props = defineProps<{ strandId: string; index: number; instruction: Extract<InstructionDto, { type: 'Key' }> }>();

const isCapturing = computed(() =>
  state.key_capture?.kind === 'Strand' && state.key_capture.strand_id === props.strandId && state.key_capture.index === props.index,
);

function onDirectionChange(dir: string) {
  editInstruction(props.strandId, props.index, { type: 'Key', key: props.instruction.key, direction: dir as KeyDirection });
}
</script>

<template>
  <span class="instruction-label">Key:</span>
  <button
    class="btn-chip key-capture-btn"
    :class="{ capturing: isCapturing }"
    @click="startKeyCapture(strandId, index)"
  >{{ isCapturing ? 'Press any key…' : instruction.key }}</button>
  <AppDropdown
    :options="['Click', 'Press', 'Release']"
    :model-value="instruction.direction"
    class-name="dd-compact"
    @update:model-value="onDirectionChange"
  />
</template>
