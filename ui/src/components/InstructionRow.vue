<script setup lang="ts">
import { computed, type Component } from 'vue';
import type { InstructionDto } from '../types';
import { isCapType, isHeaderType } from '../types';
import { beginPickup } from '../canvasDrag';
import { state } from '../store';
import { openBlockMenu } from '../contextMenu';
import { ICONS, INSTRUCTION_TYPE_ICONS } from '../icons';
import WaitFields from './fields/WaitFields.vue';
import TextFields from './fields/TextFields.vue';
import KeyFields from './fields/KeyFields.vue';
import ButtonFields from './fields/ButtonFields.vue';
import MoveMouseFields from './fields/MoveMouseFields.vue';
import ScrollFields from './fields/ScrollFields.vue';
import CommandFields from './fields/CommandFields.vue';
import CommentFields from './fields/CommentFields.vue';
import WhenRanFields from './fields/WhenRanFields.vue';
import SetVariableFields from './fields/SetVariableFields.vue';
import ChangeVariableFields from './fields/ChangeVariableFields.vue';
import BlockHeaderFields from './fields/BlockHeaderFields.vue';
import CallBlockFields from './fields/CallBlockFields.vue';
import ReturnFields from './fields/ReturnFields.vue';

const props = defineProps<{ strandId: string; index: number; instruction: InstructionDto; isFirst?: boolean; isLast?: boolean }>();

const FIELD_COMPONENTS: Record<InstructionDto['type'], Component> = {
  WhenRan: WhenRanFields,
  Wait: WaitFields,
  Text: TextFields,
  Key: KeyFields,
  Button: ButtonFields,
  MoveMouse: MoveMouseFields,
  Scroll: ScrollFields,
  Command: CommandFields,
  Comment: CommentFields,
  SetVariable: SetVariableFields,
  ChangeVariable: ChangeVariableFields,
  BlockHeader: BlockHeaderFields,
  CallBlock: CallBlockFields,
  Return: ReturnFields,
};

const fieldComponent = computed(() => FIELD_COMPONENTS[props.instruction.type]);
const typeIcon = computed(() => ICONS[INSTRUCTION_TYPE_ICONS[props.instruction.type]]);
const showIcon = computed(() => !isHeaderType(props.instruction.type));

const isRecordingTarget = computed(
  () => props.isFirst && props.strandId === state.current_macro?.recording_target_strand_id,
);

function onRowPointerDown(e: PointerEvent) {
  const target = e.target as Element | null;
  if (target?.closest?.('input, select, textarea, button, .dd-trigger, .dd-option')) return;
  if (target instanceof HTMLElement && target.isContentEditable) return;
  beginPickup(e, props.strandId, props.index);
}

function onRowContextMenu(e: MouseEvent) {
  openBlockMenu(e, props.strandId, props.index);
}
</script>

<template>
  <div
    class="instruction-row"
    :class="{ 'row-first': isFirst, 'row-last': isLast, 'instruction-row-when-ran': instruction.type === 'WhenRan', 'instruction-row-header': isHeaderType(instruction.type), 'instruction-row-cap': isCapType(instruction.type) }"
    :data-index="index"
    @pointerdown="onRowPointerDown"
    @contextmenu.prevent.stop="onRowContextMenu"
  >
    <div class="instruction-shape">
      <span v-if="isRecordingTarget" class="recording-target-dot" title="Recording target" />
      <component :is="typeIcon" v-if="showIcon" class="instruction-type-icon" />
      <div class="instruction-content">
        <component :is="fieldComponent" :strand-id="strandId" :index="index" :instruction="instruction" />
      </div>
    </div>
  </div>
</template>
