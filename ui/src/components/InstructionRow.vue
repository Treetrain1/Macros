<script setup lang="ts">
import { computed, ref, type Component } from 'vue';
import type { InstrPath, InstructionDto } from '../types';
import { isCapType, isEntryTriggerType, isHeaderType, isWrapType } from '../types';
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
import OpenAppFields from './fields/OpenAppFields.vue';
import CloseAppFields from './fields/CloseAppFields.vue';
import CommentFields from './fields/CommentFields.vue';
import WhenRanFields from './fields/WhenRanFields.vue';
import WhenBatteryDischargedToFields from './fields/WhenBatteryDischargedToFields.vue';
import WhenBatteryChargedToFields from './fields/WhenBatteryChargedToFields.vue';
import WhenTimeFields from './fields/WhenTimeFields.vue';
import WhenPowerPluggedInFields from './fields/WhenPowerPluggedInFields.vue';
import WhenPowerUnpluggedFields from './fields/WhenPowerUnpluggedFields.vue';
import SetVariableFields from './fields/SetVariableFields.vue';
import ChangeVariableFields from './fields/ChangeVariableFields.vue';
import BlockHeaderFields from './fields/BlockHeaderFields.vue';
import CallBlockFields from './fields/CallBlockFields.vue';
import ReturnFields from './fields/ReturnFields.vue';
import IfFields from './fields/IfFields.vue';
import IfElseFields from './fields/IfElseFields.vue';
import RepeatFields from './fields/RepeatFields.vue';
import ForeverFields from './fields/ForeverFields.vue';
import WhileFields from './fields/WhileFields.vue';
import EscapeLoopFields from './fields/EscapeLoopFields.vue';
import ContinueLoopFields from './fields/ContinueLoopFields.vue';

const props = defineProps<{ strandId: string; path: InstrPath; instruction: InstructionDto; isFirst?: boolean; isLast?: boolean }>();

// Every ordinary (non-wrap) instruction type renders as one puzzle-piece row
// with this field component filling `.instruction-content`. If/IfElse/
// Repeat/Forever/While are wrap/C-blocks — a stack of independently-shaped
// bars around hollow mice (see style.css's .instruction-row-wrap), rendered
// separately below.
type WrapType = 'If' | 'IfElse' | 'Repeat' | 'Forever' | 'While';
const FIELD_COMPONENTS: Record<Exclude<InstructionDto['type'], WrapType>, Component> = {
  WhenRan: WhenRanFields,
  WhenBatteryDischargedTo: WhenBatteryDischargedToFields,
  WhenBatteryChargedTo: WhenBatteryChargedToFields,
  WhenTime: WhenTimeFields,
  WhenPowerPluggedIn: WhenPowerPluggedInFields,
  WhenPowerUnplugged: WhenPowerUnpluggedFields,
  Wait: WaitFields,
  Text: TextFields,
  Key: KeyFields,
  Button: ButtonFields,
  MoveMouse: MoveMouseFields,
  Scroll: ScrollFields,
  Command: CommandFields,
  OpenApp: OpenAppFields,
  CloseApp: CloseAppFields,
  Comment: CommentFields,
  SetVariable: SetVariableFields,
  ChangeVariable: ChangeVariableFields,
  BlockHeader: BlockHeaderFields,
  CallBlock: CallBlockFields,
  Return: ReturnFields,
  EscapeLoop: EscapeLoopFields,
  ContinueLoop: ContinueLoopFields,
};

const isWrap = computed(() => isWrapType(props.instruction.type));
const fieldComponent = computed(() => (isWrap.value ? null : FIELD_COMPONENTS[props.instruction.type as Exclude<InstructionDto['type'], WrapType>]));
const typeIcon = computed(() => ICONS[INSTRUCTION_TYPE_ICONS[props.instruction.type]]);
const showIcon = computed(() => !isHeaderType(props.instruction.type));
const index = computed(() => props.path[props.path.length - 1]?.index ?? 0);

const isRecordingTarget = computed(
  () => props.isFirst && props.path.length === 1 && props.strandId === state.current_macro?.recording_target_strand_id,
);

// Walks up from `el` toward (but never past) `boundary`, looking for an
// ancestor matching `selector`. Unlike a plain `el.closest(selector)`, this
// never looks *above* the row whose own listener is running the check — a
// wrap block's outer container wraps every nested block sitting in its own
// mouth, and a plain `.closest('.wrap-mouth')` from any of those nested
// blocks' own handlers would also find whatever mouth belongs to an
// *ancestor* wrap block further up the tree (e.g. an If nested inside
// another If), incorrectly treating a click/hover on the inner block's own
// content as if it landed inside a mouth at all (confirmed: this is what
// made a nested block's own pointerdown do nothing — every nested row's
// own guard below was tripping on a mouth that belonged to some block
// *above* it, not one of its own).
function closestWithin(el: Element | null, selector: string, boundary: Element): Element | null {
  let cur = el;
  while (cur && cur !== boundary) {
    if (cur.matches(selector)) return cur;
    cur = cur.parentElement;
  }
  return null;
}

function onRowPointerDown(e: PointerEvent) {
  const target = e.target as Element | null;
  if (target?.closest?.('input, select, textarea, button, .dd-trigger, .dd-option')) return;
  if (target instanceof HTMLElement && target.isContentEditable) return;
  const currentTarget = e.currentTarget as Element;
  // Wrap blocks (If/If-Else) listen on their own outer container, not just
  // `.wrap-head-line`, so the whole outline (head line, mid bar, foot bar,
  // spine) is grabbable. But that container also wraps every nested block
  // sitting in its mouth, and pointerdown bubbles, so without this guard,
  // picking up a nested block — or even clicking genuinely empty space
  // inside a mouth — would *also* pick up the outer wrap block.
  // `.wrap-mouth`'s own box is `pointer-events: none` (only its spine
  // pseudo-elements opt back into `auto`), so a real nested `.instruction-
  // row` is only ever reached as `target` when the pointer is actually over
  // that row's own content — the spine hits `.wrap-mouth` itself as
  // `target`, not a descendant.
  const mouth = target ? closestWithin(target, '.wrap-mouth', currentTarget) : null;
  if (mouth && target !== mouth) return;
  // Genuinely empty mouth space (no spine, no nested row under the pointer)
  // has nothing pointer-events:auto to hit at all, so the browser falls all
  // the way through to the nearest ancestor that still accepts pointer
  // events — this row itself, meaning `target` ends up as the *same*
  // element the listener is bound to instead of any real descendant.
  if (target === currentTarget) return;
  beginPickup(e, props.strandId, props.path);
}

function onRowContextMenu(e: MouseEvent) {
  // Same reasoning as onRowPointerDown's guards above — right-clicking a
  // nested block, or genuinely hollow space inside a mouth, shouldn't open
  // the outer wrap block's own menu.
  const target = e.target as Element | null;
  const currentTarget = e.currentTarget as Element;
  const mouth = target ? closestWithin(target, '.wrap-mouth', currentTarget) : null;
  if (mouth && target !== mouth) return;
  if (target === currentTarget) return;
  openBlockMenu(e, props.strandId, props.path);
}

// Whether the spine (the visual strip along the left edge of a mouth,
// painted by `.wrap-mouth`'s own pseudo-elements — see style.css) is
// currently hovered, so `.wrap-spine-hover` below can re-theme the whole
// block blue the same way hovering the head/mid/foot bars already does.
// This can't be expressed as a plain `:has(.wrap-mouth:hover)` in CSS:
// `:hover` on `.wrap-mouth` is *also* true whenever a nested block sitting
// inside it is hovered (CSS hover state bubbles up through every ancestor
// regardless of `pointer-events`, so a nested row being hovered — which
// does have `pointer-events: auto` — makes its `.wrap-mouth` ancestor
// `:hover` too), and there's no way to carve that back out in pure CSS
// here: `:has()` cannot itself contain another `:has()`, which is what a
// `:not(:has(.instruction-row:hover))` refinement would need (confirmed via
// `CSS.supports`). Tracking it in JS instead sidesteps the restriction
// entirely.
const spineHovered = ref(false);

function onRowPointerOver(e: PointerEvent) {
  const target = e.target as Element | null;
  // A spine hit always lands exactly *on* its `.wrap-mouth` element — it's
  // painted by that element's own pseudo-elements (see style.css), never a
  // descendant — so this doesn't need `closestWithin`'s ancestor walk, just
  // a direct match. That distinction matters for a nested wrap block (an
  // If inside an If): the walk finds the *nearest* `.wrap-mouth` between
  // target and boundary regardless of whose it is, so hovering the *inner*
  // block's own spine (target IS the inner mouth) would otherwise also
  // satisfy the *outer* row's check, since the inner mouth is still found
  // somewhere between target and the outer boundary (confirmed: this made
  // hovering the inner block's spine highlight the outer block too).
  // Requiring `target.parentElement === currentTarget` pins the match to
  // *this* row's own direct mouth specifically.
  spineHovered.value = !!target && target.classList.contains('wrap-mouth') && target.parentElement === e.currentTarget;
}

function onRowPointerLeave() {
  spineHovered.value = false;
}
</script>

<template>
  <div
    v-if="isWrap"
    class="instruction-row instruction-row-wrap"
    :class="{ 'row-first': isFirst, 'row-last': isLast, 'wrap-spine-hover': spineHovered }"
    :data-index="index"
    @pointerdown="onRowPointerDown"
    @contextmenu.prevent.stop="onRowContextMenu"
    @pointerover="onRowPointerOver"
    @pointerleave="onRowPointerLeave"
  >
    <div class="wrap-head-line">
      <component :is="typeIcon" class="instruction-type-icon-inline" />
      <IfFields v-if="instruction.type === 'If'" part="head" :strand-id="strandId" :path="path" :instruction="instruction" />
      <IfElseFields v-else-if="instruction.type === 'IfElse'" part="head" :strand-id="strandId" :path="path" :instruction="instruction" />
      <RepeatFields v-else-if="instruction.type === 'Repeat'" part="head" :strand-id="strandId" :path="path" :instruction="instruction" />
      <ForeverFields v-else-if="instruction.type === 'Forever'" part="head" :strand-id="strandId" :path="path" :instruction="instruction" />
      <WhileFields v-else-if="instruction.type === 'While'" part="head" :strand-id="strandId" :path="path" :instruction="instruction" />
    </div>
    <IfFields v-if="instruction.type === 'If'" part="body" :strand-id="strandId" :path="path" :instruction="instruction" />
    <IfElseFields v-else-if="instruction.type === 'IfElse'" part="body" :strand-id="strandId" :path="path" :instruction="instruction" />
    <RepeatFields v-else-if="instruction.type === 'Repeat'" part="body" :strand-id="strandId" :path="path" :instruction="instruction" />
    <ForeverFields v-else-if="instruction.type === 'Forever'" part="body" :strand-id="strandId" :path="path" :instruction="instruction" />
    <WhileFields v-else-if="instruction.type === 'While'" part="body" :strand-id="strandId" :path="path" :instruction="instruction" />
    <div class="wrap-foot-bar" />
  </div>
  <div
    v-else
    class="instruction-row"
    :class="{ 'row-first': isFirst, 'row-last': isLast, 'instruction-row-when-ran': isEntryTriggerType(instruction.type), 'instruction-row-header': isHeaderType(instruction.type), 'instruction-row-cap': isCapType(instruction.type) }"
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
