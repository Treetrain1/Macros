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
  // line vs. its mouth (nested body) — see the big comment on
  // .instruction-row-wrap in style.css for why an If is one single hollow
  // bracket shape, not one filled row.
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
  <InstructionList v-else :strand-id="strandId" :base-path="bodyPath" :instructions="instruction.body" />
</template>
