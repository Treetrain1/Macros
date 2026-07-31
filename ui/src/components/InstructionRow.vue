<script setup lang="ts">
import { computed, type Component } from 'vue';
import type { InstrPath, InstructionDto } from '../types';
import { isCapType, isHeaderType, isWrapType } from '../types';
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
import IfFields from './fields/IfFields.vue';
import IfElseFields from './fields/IfElseFields.vue';

const props = defineProps<{ strandId: string; path: InstrPath; instruction: InstructionDto; isFirst?: boolean; isLast?: boolean }>();

// Every ordinary (non-wrap) instruction type renders as one puzzle-piece row
// with this field component filling `.instruction-content`. If/IfElse are
// wrap/C-blocks — handled separately below, since their shape is a hollow
// bracket (head/spine/foot), not a single filled row.
const FIELD_COMPONENTS: Record<Exclude<InstructionDto['type'], 'If' | 'IfElse'>, Component> = {
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

const isWrap = computed(() => isWrapType(props.instruction.type));
const fieldComponent = computed(() => (isWrap.value ? null : FIELD_COMPONENTS[props.instruction.type as Exclude<InstructionDto['type'], 'If' | 'IfElse'>]));
const typeIcon = computed(() => ICONS[INSTRUCTION_TYPE_ICONS[props.instruction.type]]);
const showIcon = computed(() => !isHeaderType(props.instruction.type));
const index = computed(() => props.path[props.path.length - 1]?.index ?? 0);

const isRecordingTarget = computed(
  () => props.isFirst && props.strandId === state.current_macro?.recording_target_strand_id,
);

// Attached to whichever piece is actually "the block" for pickup purposes —
// the whole outer row for an ordinary block, or the head/spine/foot bars
// individually for a wrap block (see the big CSS comment on .wrap-head).
function onRowPointerDown(e: PointerEvent) {
  const target = e.target as Element | null;
  if (target?.closest?.('input, select, textarea, button, .dd-trigger, .dd-option')) return;
  if (target instanceof HTMLElement && target.isContentEditable) return;
  beginPickup(e, props.strandId, props.path);
}

function onRowContextMenu(e: MouseEvent) {
  openBlockMenu(e, props.strandId, props.path);
}
</script>

<template>
  <div
    v-if="isWrap"
    class="instruction-row instruction-row-wrap"
    :class="{ 'row-first': isFirst, 'row-last': isLast }"
    :data-index="index"
  >
    <div class="instruction-row wrap-head" @pointerdown="onRowPointerDown" @contextmenu.prevent.stop="onRowContextMenu">
      <div class="instruction-shape">
        <component :is="typeIcon" class="instruction-type-icon" />
        <div class="instruction-content">
          <IfFields v-if="instruction.type === 'If'" part="head" :strand-id="strandId" :path="path" :instruction="instruction" />
          <IfElseFields v-else-if="instruction.type === 'IfElse'" part="head" :strand-id="strandId" :path="path" :instruction="instruction" />
        </div>
      </div>
    </div>
    <div class="wrap-body">
      <div class="wrap-spine" @pointerdown="onRowPointerDown" @contextmenu.prevent.stop="onRowContextMenu"></div>
      <div class="wrap-slots">
        <IfFields v-if="instruction.type === 'If'" part="body" :strand-id="strandId" :path="path" :instruction="instruction" />
        <IfElseFields v-else-if="instruction.type === 'IfElse'" part="body" :strand-id="strandId" :path="path" :instruction="instruction" />
      </div>
    </div>
    <div class="instruction-row wrap-foot" @pointerdown="onRowPointerDown" @contextmenu.prevent.stop="onRowContextMenu">
      <div class="instruction-shape"></div>
    </div>
  </div>
  <div
    v-else
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
        <component :is="fieldComponent" :strand-id="strandId" :path="path" :instruction="instruction" />
      </div>
    </div>
  </div>
</template>
