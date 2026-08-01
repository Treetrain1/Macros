<script setup lang="ts">
// Recursive renderer for one flat instruction list — a strand's own
// top-level instructions, or the nested body of an If/IfElse block.
// Extracted from StrandCard.vue so If/IfElseFields.vue can render their own
// nested body the same way: InstructionList -> InstructionRow -> (for an
// If/IfElse row) -> IfFields/IfElseFields -> another InstructionList, and so
// on for arbitrary nesting depth.
//
// The root carries `.instruction-list` plus a serialized `data-path` (this
// list's own `basePath` — the steps needed to reach it from the strand's
// top level) so canvasDrag.ts's updateSnapTarget can find every drop
// target on the canvas and scope its boundary scan to each one's own
// *direct* child rows, without nested bodies' rows leaking into an
// ancestor's boundary list.
import type { InstrPath, InstructionDto, PathStep } from '../types';
import InstructionRow from './InstructionRow.vue';
import { beginPickup } from '../canvasDrag';

const props = defineProps<{ strandId: string; basePath: PathStep[]; instructions: InstructionDto[] }>();

function childPath(index: number): InstrPath {
  return [...props.basePath, { index }];
}

function onEmptyHintPointerDown(e: PointerEvent) {
  beginPickup(e, props.strandId, childPath(0));
}

// Only a strand's own top-level empty hint gets the "Empty — drag..." text
// (and is grabbable, to reposition the empty strand) — an empty If/IfElse
// body is just blank space inside the bracket, matching Scratch/PenguinMod;
// it's still a real drop target (updateSnapTarget falls back to this
// container's own rect when it has no rows), just with no placeholder copy.
const isTopLevel = props.basePath.length === 0;
</script>

<template>
  <div class="strand-body instruction-list" :data-strand-id="strandId" :data-path="JSON.stringify(basePath)">
    <div v-if="instructions.length === 0 && isTopLevel" class="strand-empty-hint" @pointerdown="onEmptyHintPointerDown">
      Empty — drag an instruction here from the sidebar.
    </div>
    <InstructionRow
      v-for="(ins, i) in instructions"
      :key="i"
      :strand-id="strandId"
      :path="childPath(i)"
      :instruction="ins"
      :is-first="i === 0"
      :is-last="i === instructions.length - 1"
    />
  </div>
</template>
