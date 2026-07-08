<script setup lang="ts">
import { computed, type Component } from 'vue';
import { Move } from 'lucide-vue-next';
import type { InstructionDto } from '../types';
import { beginPickup } from '../canvasDrag';
import WaitFields from './fields/WaitFields.vue';
import TextFields from './fields/TextFields.vue';
import KeyFields from './fields/KeyFields.vue';
import ButtonFields from './fields/ButtonFields.vue';
import MoveMouseFields from './fields/MoveMouseFields.vue';
import ScrollFields from './fields/ScrollFields.vue';
import CommandFields from './fields/CommandFields.vue';
import CommentFields from './fields/CommentFields.vue';

const props = defineProps<{ strandId: string; index: number; instruction: InstructionDto }>();

const FIELD_COMPONENTS: Record<InstructionDto['type'], Component> = {
  Wait: WaitFields,
  Text: TextFields,
  Key: KeyFields,
  Button: ButtonFields,
  MoveMouse: MoveMouseFields,
  Scroll: ScrollFields,
  Command: CommandFields,
  Comment: CommentFields,
};

const fieldComponent = computed(() => FIELD_COMPONENTS[props.instruction.type]);

function onGripPointerDown(e: PointerEvent) {
  beginPickup(e, props.strandId, props.index);
}
</script>

<template>
  <div class="instruction-row" :data-index="index">
    <span class="row-grip" title="Drag to move or detach" @pointerdown="onGripPointerDown"><Move /></span>
    <div class="instruction-content">
      <component :is="fieldComponent" :strand-id="strandId" :index="index" :instruction="instruction" />
    </div>
  </div>
</template>
