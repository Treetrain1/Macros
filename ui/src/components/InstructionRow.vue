<script setup lang="ts">
import { computed, type Component } from 'vue';
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

const props = defineProps<{ strandId: string; index: number; instruction: InstructionDto; isFirst?: boolean; isLast?: boolean }>();

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

function onRowPointerDown(e: PointerEvent) {
  const target = e.target as Element | null;
  if (target?.closest?.('input, select, textarea, button, .dd-trigger, .dd-option')) return;
  if (target instanceof HTMLElement && target.isContentEditable) return;
  beginPickup(e, props.strandId, props.index);
}
</script>

<template>
  <div class="instruction-row" :class="{ 'row-first': isFirst, 'row-last': isLast }" :data-index="index" @pointerdown="onRowPointerDown">
    <div class="instruction-shape">
      <div class="instruction-content">
        <component :is="fieldComponent" :strand-id="strandId" :index="index" :instruction="instruction" />
      </div>
    </div>
  </div>
</template>
