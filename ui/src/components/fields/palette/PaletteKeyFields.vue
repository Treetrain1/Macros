<script setup lang="ts">
import AutosizeInput from '../../AutosizeInput.vue';
import AppDropdown from '../../AppDropdown.vue';
import type { InstructionDto, KeyDirection } from '../../../types';

const props = defineProps<{ instruction: Extract<InstructionDto, { type: 'Key' }> }>();

// The real block captures the next keypress via the backend (startKeyCapture)
// — meaningless for a prefab with no real strand/index, so this is a plain
// text field instead of a capture button.
function onKeyChange(v: string) {
  props.instruction.key = v;
}
function onDirectionChange(dir: string) {
  props.instruction.direction = dir as KeyDirection;
}
</script>

<template>
  <span class="instruction-label">Key:</span>
  <AutosizeInput :model-value="instruction.key" :min-chars="2" @update:model-value="onKeyChange" />
  <AppDropdown
    :options="['Click', 'Press', 'Release']"
    :model-value="instruction.direction"
    class-name="dd-compact"
    @update:model-value="onDirectionChange"
  />
</template>
