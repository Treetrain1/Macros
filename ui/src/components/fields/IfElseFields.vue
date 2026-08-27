<script setup lang="ts">
import { computed } from 'vue';
import { ValueBlock } from 'blockstitch';
import { InstructionList } from 'blockstitch';
import { bodyBasePath, fieldLocation } from '../../types';
import type { InstrPath, InstructionDto } from '../../types';

const props = defineProps<{
  strandId: string;
  path: InstrPath;
  instruction: Extract<InstructionDto, { type: 'IfElse' }>;
  // See IfFields.vue's header comment — rendered separately for the wrap
  // block's head line vs. its mouth (nested body).
  part: 'head' | 'body';
}>();

const thenPath = computed(() => bodyBasePath(props.path, 0));
const elsePath = computed(() => bodyBasePath(props.path, 1));
</script>

<template>
  <template v-if="part === 'head'">
    <span class="instruction-label">if</span>
    <ValueBlock :location="fieldLocation(strandId, path, 'Condition')" :value="instruction.condition" />
    <span class="instruction-label">then</span>
  </template>
  <template v-else>
    <div class="wrap-mouth">
      <InstructionList :strand-id="strandId" :base-path="thenPath" :instructions="instruction.then_body" />
    </div>
    <div class="wrap-mid-bar"><span class="instruction-label">else</span></div>
    <div class="wrap-mouth">
      <InstructionList :strand-id="strandId" :base-path="elsePath" :instructions="instruction.else_body" />
    </div>
  </template>
</template>
