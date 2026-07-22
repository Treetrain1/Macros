<script setup lang="ts">
// "Make a Block"/"Edit Block" popup — a centered live prototype preview
// (labels + input ovals) the user builds up by inserting pieces, plus a
// return-type choice, teleported to <body> like MakeVariableDialog.vue.
// Doubles as both "Make a Block" (no `editTarget`) and "Edit Block" (from a
// prefab's context menu, see ContextMenu.vue/blockDialogs.ts) — editing
// works on a local copy of `pieces`/`returnsValue` and only writes back via
// createBlock/editBlock on OK, so Cancel is a true no-op.
import { computed, nextTick, reactive, ref, watch } from 'vue';
import { ChevronLeft, ChevronRight, X } from 'lucide-vue-next';
import { createBlock, editBlock } from '../tauri';
import type { BlockDefDto, BlockPieceDto } from '../types';

const props = defineProps<{ editTarget: BlockDefDto | null }>();
const emit = defineEmits<{ close: [] }>();

const isEdit = computed(() => !!props.editTarget);

// Stable per-piece id — preserved verbatim for every existing piece (so
// `edit_block` can tell a rename apart from a remove+add, see types.ts's
// BlockPieceDto comment), freshly generated only for a piece created in
// this session (`addPiece`).
function newPieceId(): string {
  return crypto.randomUUID?.() ?? `p${Math.random().toString(36).slice(2)}`;
}

const pieces = reactive<BlockPieceDto[]>(
  props.editTarget ? props.editTarget.pieces.map(p => ({ ...p })) : [{ kind: 'Label', id: newPieceId(), text: 'block name' }],
);
const returnsValue = ref(props.editTarget?.returns_value ?? false);
const error = ref<string | null>(null);
const submitting = ref(false);

const editingIndex = ref<number | null>(null);
const editingText = ref('');
const editInputEl = ref<HTMLInputElement | null>(null);
// The piece the remove/move-left/move-right toolbar floats above. Separate
// from `editingIndex` (which only tracks the live rename text field) so the
// toolbar stays put once a rename commits instead of disappearing.
const selectedIndex = ref<number | null>(null);

// Starts past however many inputs already exist so a freshly-inserted
// input's default name doesn't collide with an existing "valueN" (which
// would otherwise immediately trip the uniqueness check on OK).
let nextInputSeq = pieces.filter(p => p.kind === 'Input').length + 1;

function pieceText(piece: BlockPieceDto): string {
  return piece.kind === 'Label' ? (piece.text || '(label)') : piece.name;
}

function startEditing(i: number) {
  const piece = pieces[i];
  editingIndex.value = i;
  selectedIndex.value = i;
  editingText.value = piece.kind === 'Label' ? piece.text : piece.name;
}

watch(editingIndex, async i => {
  if (i === null) return;
  await nextTick();
  editInputEl.value?.focus();
  editInputEl.value?.select();
});

function commitEditing() {
  if (editingIndex.value === null) return;
  const piece = pieces[editingIndex.value];
  const text = editingText.value.trim();
  if (piece.kind === 'Label') {
    piece.text = text;
  } else if (text) {
    piece.name = text;
  }
  editingIndex.value = null;
}

function addPiece(kind: 'Label' | 'Input') {
  const piece: BlockPieceDto =
    kind === 'Label' ? { kind: 'Label', id: newPieceId(), text: 'label' } : { kind: 'Input', id: newPieceId(), name: `value${nextInputSeq++}` };
  pieces.push(piece);
  const index = pieces.length - 1;
  if (kind === 'Input') {
    // New inputs land pre-selected for immediate renaming, matching "click
    // the name to edit it" for every other piece.
    nextTick(() => startEditing(index));
  } else {
    selectedIndex.value = index;
  }
}

function removePiece(i: number) {
  pieces.splice(i, 1);
  if (editingIndex.value === i) editingIndex.value = null;
  if (selectedIndex.value === i) selectedIndex.value = null;
  else if (selectedIndex.value !== null && selectedIndex.value > i) selectedIndex.value--;
}

function movePiece(i: number, dir: -1 | 1) {
  const j = i + dir;
  if (j < 0 || j >= pieces.length) return;
  [pieces[i], pieces[j]] = [pieces[j], pieces[i]];
  selectedIndex.value = j;
}

async function onOk() {
  if (submitting.value) return;
  const flatLabel = pieces.filter((p): p is Extract<BlockPieceDto, { kind: 'Label' }> => p.kind === 'Label').map(p => p.text.trim()).join(' ').trim();
  if (!flatLabel) {
    error.value = 'Give the block a name';
    return;
  }
  const inputNames = pieces.filter((p): p is Extract<BlockPieceDto, { kind: 'Input' }> => p.kind === 'Input').map(p => p.name.trim());
  if (inputNames.some(n => !n)) {
    error.value = 'Every input needs a name';
    return;
  }
  if (new Set(inputNames).size !== inputNames.length) {
    error.value = 'Input names must be unique';
    return;
  }
  submitting.value = true;
  try {
    const snapshot = pieces.map(p => ({ ...p }));
    if (props.editTarget) {
      await editBlock(props.editTarget.id, snapshot, returnsValue.value);
    } else {
      await createBlock(snapshot, returnsValue.value);
    }
    emit('close');
  } catch (e) {
    error.value = String(e);
  } finally {
    submitting.value = false;
  }
}

function onCancel() {
  emit('close');
}
</script>

<template>
  <Teleport to="body">
    <div class="modal-overlay" @pointerdown.self="onCancel">
      <div class="modal-panel make-block-panel">
        <h2 class="modal-title">{{ isEdit ? 'Edit Block' : 'Make a Block' }}</h2>

        <div class="make-block-canvas">
          <div class="make-block-preview" @pointerdown.self="selectedIndex = null">
            <template v-for="(piece, i) in pieces" :key="piece.id">
              <span
                class="make-block-piece"
                :class="{ 'make-block-piece-input': piece.kind === 'Input', 'make-block-piece-selected': selectedIndex === i }"
              >
                <div v-if="selectedIndex === i" class="make-block-piece-toolbar">
                  <button type="button" class="make-block-piece-move" title="Move left" :disabled="i === 0" @click.stop="movePiece(i, -1)">
                    <ChevronLeft />
                  </button>
                  <button type="button" class="make-block-piece-remove" title="Remove" @click.stop="removePiece(i)">
                    <X />
                  </button>
                  <button
                    type="button"
                    class="make-block-piece-move"
                    title="Move right"
                    :disabled="i === pieces.length - 1"
                    @click.stop="movePiece(i, 1)"
                  >
                    <ChevronRight />
                  </button>
                </div>
                <span class="make-block-piece-field">
                  <span
                    class="make-block-piece-text"
                    :class="{ 'make-block-piece-text-hidden': editingIndex === i }"
                    @click="startEditing(i)"
                  >{{ pieceText(piece) }}</span>
                  <input
                    v-if="editingIndex === i"
                    ref="editInputEl"
                    type="text"
                    class="make-block-piece-input-el"
                    v-model="editingText"
                    @blur="commitEditing"
                    @keydown.enter="commitEditing"
                    @keydown.esc="editingIndex = null"
                  />
                </span>
              </span>
            </template>
          </div>
        </div>

        <div class="make-block-add-row">
          <button type="button" class="make-block-add-btn" @click="addPiece('Input')">
            <span class="make-block-add-preview make-block-add-preview-input">123</span>
            <span class="make-block-add-text">
              <span class="make-block-add-title">Add an input</span>
              <span class="make-block-add-sub">number or text</span>
            </span>
          </button>
          <button type="button" class="make-block-add-btn" @click="addPiece('Label')">
            <span class="make-block-add-preview make-block-add-preview-label">Abc</span>
            <span class="make-block-add-text">
              <span class="make-block-add-title">Add a label</span>
            </span>
          </button>
        </div>

        <div class="make-block-return-row">
          <span class="instruction-label">This block:</span>
          <label class="make-block-radio">
            <input type="radio" :checked="!returnsValue" @change="returnsValue = false" /> doesn't return a value
          </label>
          <label class="make-block-radio">
            <input type="radio" :checked="returnsValue" @change="returnsValue = true" /> returns a value
          </label>
        </div>

        <span v-if="error" class="invalid-hint">{{ error }}</span>
        <div class="modal-actions">
          <button type="button" @click="onCancel">Cancel</button>
          <button type="button" class="btn-primary" :disabled="submitting" @click="onOk">OK</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>
