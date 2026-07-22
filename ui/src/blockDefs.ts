// Ephemeral, per-macro palette state for "My Blocks" prefabs — the
// live-editable arg list a CallBlock/Call prefab currently carries while it
// sits in the sidebar, before being dragged out. Unlike paletteState.ts's
// registry-backed prefabs (a fixed set of operators known ahead of time),
// custom blocks are per-macro and dynamic, so this is keyed by block id and
// kept in sync with the current macro's `block_defs` (including resizing an
// existing arg list when an input is added/removed) rather than being a
// fixed Record built once at module load — deliberately *not* following
// paletteState.ts's "never tied to any particular macro" rule, since a
// block's very existence (and its input count) is macro-scoped.
import { reactive, watch } from 'vue';
import { state } from './store';
import { numberValue, blockInputNames, findBlockDef } from './types';
import type { InstructionDto, ValueDto } from './types';

export const paletteCallArgs = reactive<Record<string, ValueDto[]>>({});

watch(
  () => state.current_macro?.block_defs,
  defs => {
    const list = defs ?? [];
    const ids = new Set(list.map(d => d.id));
    for (const id of Object.keys(paletteCallArgs)) {
      if (!ids.has(id)) delete paletteCallArgs[id];
    }
    for (const def of list) {
      const count = blockInputNames(def).length;
      const existing = paletteCallArgs[def.id] ?? [];
      paletteCallArgs[def.id] = Array.from({ length: count }, (_, i) => existing[i] ?? numberValue(0));
    }
  },
  { immediate: true, deep: true },
);

function currentArgs(blockId: string): ValueDto[] {
  const def = findBlockDef(state.current_macro, blockId);
  const count = def ? blockInputNames(def).length : (paletteCallArgs[blockId]?.length ?? 0);
  const existing = paletteCallArgs[blockId] ?? [];
  return Array.from({ length: count }, (_, i) => existing[i] ?? numberValue(0));
}

/** The full `ValueDto` a "My Blocks" reporter prefab currently represents —
 * what lands in a value slot/floating card when it's dragged out. Mirrors
 * paletteState.ts's `paletteValueFor`, just for `Call:<blockId>` kinds
 * (which that registry-driven function can't build, since arity is dynamic
 * per block rather than fixed per `ValueKind`) — see valueDrag.ts's
 * `resolveFreshValue`, the one place that dispatches between the two. */
export function paletteCallValueFor(blockId: string): ValueDto {
  return { kind: 'Call', block_id: blockId, args: currentArgs(blockId), saved: numberValue(0) };
}

/** The full `InstructionDto` a "My Blocks" command prefab currently
 * represents — counterpart to `paletteCallValueFor`, consumed by
 * canvasDrag.ts's `beginPaletteDrag`/drop handling in place of
 * `clonePaletteInstruction` (which, same reasoning, can't resolve a
 * per-block-id `CallBlock` from `insType` alone). */
export function paletteCallInstructionFor(blockId: string): InstructionDto {
  return { type: 'CallBlock', block_id: blockId, args: currentArgs(blockId) };
}
