<script setup lang="ts">
// Renders a `CallBlock` instruction's prototype dynamically from its
// `BlockDef` (labels as plain text, one `ValueBlock` per declared input) —
// there's no fixed shape to hardcode, unlike every other instruction's
// *Fields component, since it depends on whichever block this call names.
import { computed } from 'vue';
import { state } from '../../store';
import { fieldLocation, findBlockDef } from '../../types';
import { ValueBlock } from 'blockstitch';
import type { InstrPath, InstructionDto } from '../../types';

const props = defineProps<{ strandId: string; path: InstrPath; instruction: Extract<InstructionDto, { type: 'CallBlock' }> }>();

const def = computed(() => findBlockDef(state.current_macro, props.instruction.block_id));

// Pairs each prototype piece with the arg index it addresses (labels get
// -1 — nothing to look up in `args`). Mirrors ValueBlock.vue's `callPieces`.
const pieces = computed(() => {
  if (!def.value) return [];
  let argIndex = 0;
  return def.value.pieces.map(piece => (piece.kind === 'Input' ? { piece, argIndex: argIndex++ } : { piece, argIndex: -1 }));
});
</script>

<template>
  <span v-if="!def" class="instruction-label">(deleted block)</span>
  <template v-else v-for="(item, i) in pieces" :key="i">
    <span v-if="item.piece.kind === 'Label'" class="instruction-label">{{ item.piece.text }}</span>
    <ValueBlock
      v-else
      :location="fieldLocation(strandId, path, `CallArg:${item.argIndex}`)"
      :value="instruction.args[item.argIndex] ?? { kind: 'Number', value: 0 }"
    />
  </template>
</template>
