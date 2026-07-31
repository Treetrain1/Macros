<script setup lang="ts">
import { computed } from 'vue';
import ValueBlock from '../ValueBlock.vue';
import InstructionList from '../InstructionList.vue';
import { bodyBasePath, fieldLocation } from '../../types';
import type { InstrPath, InstructionDto } from '../../types';

const props = defineProps<{ strandId: string; path: InstrPath; instruction: Extract<InstructionDto, { type: 'IfElse' }> }>();

const thenPath = computed(() => bodyBasePath(props.path, 0));
const elsePath = computed(() => bodyBasePath(props.path, 1));
</script>

<template>
  <div class="wrap-header">
    <span class="instruction-label">if</span>
    <ValueBlock :location="fieldLocation(strandId, path, 'Condition')" :value="instruction.condition" />
    <span class="instruction-label">then</span>
  </div>
  <div class="wrap-slot">
    <InstructionList :strand-id="strandId" :base-path="thenPath" :instructions="instruction.then_body" />
  </div>
  <div class="wrap-divider">
    <span class="instruction-label">else</span>
  </div>
  <div class="wrap-slot">
    <InstructionList :strand-id="strandId" :base-path="elsePath" :instructions="instruction.else_body" />
  </div>
</template>
