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
// wrap/C-blocks — a stack of independently-shaped bars around hollow mice
// (see style.css's .instruction-row-wrap), rendered separately below.
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
  () => props.isFirst && props.path.length === 1 && props.strandId === state.current_macro?.recording_target_strand_id,
);

function onRowPointerDown(e: PointerEvent) {
  const target = e.target as Element | null;
  if (target?.closest?.('input, select, textarea, button, .dd-trigger, .dd-option')) return;
  if (target instanceof HTMLElement && target.isContentEditable) return;
  // Wrap blocks (If/If-Else) listen on their own outer container, not just
  // `.wrap-head-line`, so the whole outline (head line, mid bar, foot bar,
  // spine) is grabbable — matching the blue hover highlight covering the
  // same area (see .instruction-row-wrap's :has() rule in style.css). But
  // that container also wraps every nested block sitting in its mouth, and
  // pointerdown bubbles, so without these guards, picking up a nested block
  // — or even clicking genuinely empty space inside a mouth — would *also*
  // pick up the outer wrap block.
  // `.wrap-mouth`'s own box is `pointer-events: none` (only its spine
  // pseudo-elements opt back into `auto`), so a real nested `.instruction-
  // row` is only ever reached as `target` when the pointer is actually over
  // that row's own content — the spine hits `.wrap-mouth` itself as
  // `target`, not a descendant.
  const mouth = target?.closest?.('.wrap-mouth');
  if (mouth && target !== mouth) return;
  // Genuinely empty mouth space (no spine, no nested row under the pointer)
  // has nothing pointer-events:auto to hit at all, so the browser falls all
  // the way through to the nearest ancestor that still accepts pointer
  // events — this row itself, meaning `target` ends up as the *same*
  // element the listener is bound to instead of any real descendant.
  if (target === e.currentTarget) return;
  beginPickup(e, props.strandId, props.path);
}

function onRowContextMenu(e: MouseEvent) {
  // Same reasoning as onRowPointerDown's guards above — right-clicking a
  // nested block, or genuinely hollow space inside a mouth, shouldn't open
  // the outer wrap block's own menu.
  const target = e.target as Element | null;
  const mouth = target?.closest?.('.wrap-mouth');
  if (mouth && target !== mouth) return;
  if (target === e.currentTarget) return;
  openBlockMenu(e, props.strandId, props.path);
}
</script>

<template>
  <div
    v-if="isWrap"
    class="instruction-row instruction-row-wrap"
    :class="{ 'row-first': isFirst, 'row-last': isLast }"
    :data-index="index"
    @pointerdown="onRowPointerDown"
    @contextmenu.prevent.stop="onRowContextMenu"
  >
    <div class="wrap-head-line">
      <component :is="typeIcon" class="instruction-type-icon-inline" />
      <IfFields v-if="instruction.type === 'If'" part="head" :strand-id="strandId" :path="path" :instruction="instruction" />
      <IfElseFields v-else-if="instruction.type === 'IfElse'" part="head" :strand-id="strandId" :path="path" :instruction="instruction" />
    </div>
    <IfFields v-if="instruction.type === 'If'" part="body" :strand-id="strandId" :path="path" :instruction="instruction" />
    <IfElseFields v-else-if="instruction.type === 'IfElse'" part="body" :strand-id="strandId" :path="path" :instruction="instruction" />
    <div class="wrap-foot-bar" />
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
