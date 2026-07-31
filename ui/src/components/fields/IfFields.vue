<script setup lang="ts">
import { computed } from 'vue';
import ValueBlock from '../ValueBlock.vue';
import InstructionList from '../InstructionList.vue';
import { bodyBasePath, fieldLocation } from '../../types';
import type { InstrPath, InstructionDto } from '../../types';

const props = defineProps<{
  strandId: string;
  path: InstrPath;
  instruction: Extract<InstructionDto, { type: 'If' }>;
  // Rendered separately by InstructionRow.vue into the wrap block's head
  // bar vs. its body/slot area — see the big comment on .wrap-head in
  // style.css for why an If isn't one single filled row.
  part: 'head' | 'body';
}>();

const bodyPath = computed(() => bodyBasePath(props.path, 0));
</script>

<template>
  <template v-if="part === 'head'">
    <span class="instruction-label">if</span>
    <ValueBlock :location="fieldLocation(strandId, path, 'Condition')" :value="instruction.condition" />
    <span class="instruction-label">then</span>
  </template>
  <div v-else class="wrap-slot">
    <InstructionList :strand-id="strandId" :base-path="bodyPath" :instructions="instruction.body" />
  </div>
</template>
