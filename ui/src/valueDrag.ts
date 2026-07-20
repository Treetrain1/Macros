// Pointer-drag state machine for value blocks (Number/Text/Add/Sub/Mul/Div/Random),
// deliberately separate from canvasDrag.ts's strand drag/snap machinery: a
// value block can be dropped into a field/subfield slot or onto open canvas,
// but must never attach to a strand's instruction list — keeping this in its
// own module means that's structural (this file simply never imports
// addInstruction/addStrand/mergeStrand/splitStrand), not just a runtime
// check that could bit-rot.
//
// Same imperative approach as canvasDrag.ts (real-time pointer tracking +
// live DOM hit-testing), simplified where the strand system's extra
// complexity doesn't apply here: existing blocks are always dragged as a
// cloned ghost (never a real-DOM-node move), and take/put/create calls only
// fire at drop time, not at pickup — there's no split-at-pickup equivalent
// needed since nothing needs to preserve Vue component identity mid-drag.
import { ref } from 'vue';
import { state } from './store';
import { capturePointer, clientToCanvas, getCanvasZoom, isOverSidebar, setSidebarArmed } from './canvasDrag';
import { createFloatingValue, moveFloatingValue, previewValue, putValue, removeFloatingValue, takeValue } from './tauri';
import { paletteValueFor } from './paletteState';
import { numberValue } from './types';
import { locationsEqual } from './invalidField';
import type { ValueDto, ValueKind, ValueLocationDto } from './types';

// Field/subfield locations whose current leaf (Number/Text) got there by
// being dropped as its own block — from the sidebar's Number/Text prefab, or
// an existing block dragged over — rather than just being the field's
// unremarkable native content. ValueBlock.vue's `boxed` consults this so a
// dropped-in leaf keeps its `.value-card-shape` capsule look (and stays
// pickup-draggable) instead of flattening into a bare input like an
// ordinary field. An `Op` block doesn't need an entry (already boxed by
// kind), but dropping one over a previously-marked location still clears the mark
// via markOrUnmark below, so a stale entry can't linger under new content.
// Purely a frontend affordance — not persisted, so it won't survive a
// reload or reflect an undo/redo that changes a slot's content underneath it.
export const capsuleLocations = ref<ValueLocationDto[]>([]);

export function isCapsuleLocation(location: ValueLocationDto): boolean {
  return capsuleLocations.value.some(l => locationsEqual(l, location));
}

function markOrUnmark(location: ValueLocationDto, value: ValueDto) {
  const isLeaf = value.kind === 'Number' || value.kind === 'Text';
  const already = isCapsuleLocation(location);
  if (isLeaf && !already) capsuleLocations.value.push(location);
  else if (!isLeaf && already) capsuleLocations.value = capsuleLocations.value.filter(l => !locationsEqual(l, location));
}

function unmarkCapsule(location: ValueLocationDto) {
  if (isCapsuleLocation(location)) capsuleLocations.value = capsuleLocations.value.filter(l => !locationsEqual(l, location));
}

// While an existing field/subfield value is mid-drag, the ValueBlock at its
// origin location renders this instead of its real prop — the exact value
// take_value would leave behind (an operator's `saved` operand, or a bare
// zero for a leaf; see src-tauri/src/commands.rs's take_value) — so the slot
// reads correctly the instant the block lifts off, rather than sitting blank
// until the drop's async take/put round trip lands and Vue re-renders it for
// real. Consulted by ValueBlock.vue via locationsEqual.
export const dragReveal = ref<{ location: ValueLocationDto; value: ValueDto } | null>(null);

const EVAL_PREVIEW_TIMEOUT_MS = 2500;

// A pointerdown-then-pointerup on an operator block that never crossed the
// drag threshold (see `onPointerUp` below) samples-evaluates that exact node
// and shows the result here — ValueBlock.vue renders a small tooltip beside
// whichever block's location matches. Auto-dismisses after a timeout, and
// any new pickup clears a stale one immediately rather than leaving it to
// linger under a different block.
export const evalPreview = ref<{ location: ValueLocationDto; text: string; error: boolean } | null>(null);
let evalPreviewTimer: ReturnType<typeof setTimeout> | null = null;

function showEvalPreview(location: ValueLocationDto, text: string, error: boolean) {
  if (evalPreviewTimer) clearTimeout(evalPreviewTimer);
  evalPreview.value = { location, text, error };
  evalPreviewTimer = setTimeout(() => {
    evalPreview.value = null;
    evalPreviewTimer = null;
  }, EVAL_PREVIEW_TIMEOUT_MS);
}

function clearEvalPreview() {
  if (evalPreviewTimer) {
    clearTimeout(evalPreviewTimer);
    evalPreviewTimer = null;
  }
  evalPreview.value = null;
}

async function previewClickedOperator(location: ValueLocationDto, value: ValueDto) {
  try {
    const text = await previewValue(value);
    showEvalPreview(location, text, false);
  } catch (err) {
    showEvalPreview(location, String(err), true);
  }
}

type ValueDragSource =
  | { kind: 'existing'; location: ValueLocationDto; value: ValueDto }
  | { kind: 'fresh'; valueKind: ValueKind };

interface ValueDragCandidate {
  pointerId: number;
  startX: number;
  startY: number;
  anchorEl: HTMLElement;
  source: ValueDragSource;
}
let valueDragCandidate: ValueDragCandidate | null = null;

interface ValueDragState {
  pointerId: number;
  offsetX: number;
  offsetY: number;
  ghostEl: HTMLElement;
  source: ValueDragSource;
  dropTarget: { el: HTMLElement; location: ValueLocationDto } | null;
  overTrash: boolean;
  // The real floating-card element being dragged, hidden for the drag's
  // duration so it doesn't sit fully visible next to the ghost that's
  // tracking the pointer — unlike an embedded field/subfield (which has
  // dragReveal to show a placeholder in its place), a whole floating block
  // has nothing left behind to show; the card itself is what's moving.
  hiddenAnchorEl: HTMLElement | null;
}
let valueDrag: ValueDragState | null = null;

/** Picking up a block that already exists — embedded in a field/subfield,
 * or parked floating on canvas. `anchorEl` is that block's own rendered
 * root (`.value-card-shape`), cloned into the drag ghost once the pointer
 * actually moves past the click-vs-drag threshold. */
export function beginValuePickup(e: PointerEvent, location: ValueLocationDto, value: ValueDto, anchorEl: HTMLElement) {
  if (state.recording_phase.phase === 'Active') return;
  if (e.button !== undefined && e.button !== 0) return;
  e.preventDefault();
  capturePointer(e);
  clearEvalPreview();
  valueDragCandidate = { pointerId: e.pointerId, startX: e.clientX, startY: e.clientY, anchorEl, source: { kind: 'existing', location, value } };
}

/** Dragging a brand-new block off the sidebar's "Operator" section.
 * `anchorEl` is that kind's hidden ghost template (see InstructionSidebar's
 * existing `registerGhost` pattern for instruction rows — same idea). */
export function beginValuePaletteDrag(e: PointerEvent, valueKind: ValueKind, anchorEl: HTMLElement) {
  if (state.recording_phase.phase === 'Active') return;
  if (e.button !== undefined && e.button !== 0) return;
  e.preventDefault();
  capturePointer(e);
  clearEvalPreview();
  valueDragCandidate = { pointerId: e.pointerId, startX: e.clientX, startY: e.clientY, anchorEl, source: { kind: 'fresh', valueKind } };
}

let ghostRafPending = false;
let lastPointerEvent: PointerEvent | null = null;
function positionGhost(e: PointerEvent) {
  lastPointerEvent = e;
  if (ghostRafPending) return;
  ghostRafPending = true;
  requestAnimationFrame(() => {
    ghostRafPending = false;
    if (!valueDrag || !lastPointerEvent) return;
    const tx = lastPointerEvent.clientX - valueDrag.offsetX;
    const ty = lastPointerEvent.clientY - valueDrag.offsetY;
    valueDrag.ghostEl.style.transform = `translate(${tx}px, ${ty}px) scale(${getCanvasZoom()})`;
    valueDrag.ghostEl.style.transformOrigin = '0 0';
  });
}

function startValueDrag(e: PointerEvent, candidate: ValueDragCandidate) {
  const rect = candidate.anchorEl.getBoundingClientRect();
  const ghost = document.createElement('div');
  ghost.className = 'value-drag-ghost';
  ghost.appendChild(candidate.anchorEl.cloneNode(true) as HTMLElement);
  document.body.appendChild(ghost);

  // A palette drag's anchorEl is the sidebar's hidden off-screen template
  // (InstructionSidebar renders it at `left: -9999px; top: -9999px`), so its
  // rect.left/top are that off-screen position, not anything cursor-relative
  // — deriving the pointer offset from them (as the 'existing' case does)
  // sent both the ghost and, since drop coordinates are computed by
  // subtracting this same offset, the new block itself flying off toward
  // -9999,-9999. Center the ghost under the pointer instead; only picking up
  // a real, on-screen block has a meaningful click-relative offset to keep.
  const isFresh = candidate.source.kind === 'fresh';
  const offsetX = isFresh ? rect.width / 2 : e.clientX - rect.left;
  const offsetY = isFresh ? rect.height / 2 : e.clientY - rect.top;

  let hiddenAnchorEl: HTMLElement | null = null;
  if (candidate.source.kind === 'existing') {
    const { location, value } = candidate.source;
    // A whole floating block (root, empty path) has no field left behind to
    // reveal anything in — the card itself is what's moving/disappearing, so
    // hide the real one (already cloned into the ghost above) instead of
    // leaving it sitting fully visible at its old spot for the whole drag.
    if (location.kind === 'Floating' && location.path.length === 0) {
      candidate.anchorEl.style.visibility = 'hidden';
      hiddenAnchorEl = candidate.anchorEl;
    } else {
      dragReveal.value = { location, value: value.kind === 'Op' ? value.saved : numberValue(0) };
    }
  }

  valueDrag = {
    pointerId: candidate.pointerId,
    offsetX,
    offsetY,
    ghostEl: ghost,
    source: candidate.source,
    dropTarget: null,
    overTrash: false,
    hiddenAnchorEl,
  };
  positionGhost(e);
}

function isSelfOrDescendant(candidate: ValueLocationDto, root: ValueLocationDto): boolean {
  if (candidate.kind !== root.kind) return false;
  if (candidate.kind === 'Field' && root.kind === 'Field') {
    if (candidate.strand_id !== root.strand_id || candidate.index !== root.index || candidate.field_id !== root.field_id) return false;
  } else if (candidate.kind === 'Floating' && root.kind === 'Floating') {
    if (candidate.floating_id !== root.floating_id) return false;
  } else {
    return false;
  }
  return candidate.path.length >= root.path.length && root.path.every((p, i) => candidate.path[i] === p);
}

/** Walks up from the topmost element under the pointer to the nearest
 * `[data-value-location]` block, skipping only the dragged block's own
 * subtree (can't drop a block into itself or one of its own operands). A
 * floating card's own root (path `[]`) is a valid target like any other —
 * dropping onto one swaps its content in place (see `swapValue` in
 * onPointerUp), ejecting whatever it held as its own new floating card. */
function findDropTarget(clientX: number, clientY: number, exclude: ValueLocationDto | null): { el: HTMLElement; location: ValueLocationDto } | null {
  let el: Element | null = document.elementFromPoint(clientX, clientY);
  while (el) {
    const match = el.closest<HTMLElement>('[data-value-location]');
    if (!match) return null;
    let location: ValueLocationDto;
    try {
      location = JSON.parse(match.dataset.valueLocation ?? '');
    } catch {
      return null;
    }
    const isSelf = !!exclude && isSelfOrDescendant(location, exclude);
    if (!isSelf) {
      return { el: match, location };
    }
    el = match.parentElement;
  }
  return null;
}

let highlightedEl: HTMLElement | null = null;
function setDropHighlight(target: { el: HTMLElement } | null) {
  const el = target?.el ?? null;
  if (highlightedEl === el) return;
  highlightedEl?.classList.remove('value-drop-target');
  highlightedEl = el;
  highlightedEl?.classList.add('value-drop-target');
}
function clearDropHighlight() {
  setDropHighlight(null);
}

function onPointerMove(e: PointerEvent) {
  if (valueDrag && e.pointerId === valueDrag.pointerId) {
    positionGhost(e);
    if (isOverSidebar(e)) {
      valueDrag.overTrash = true;
      valueDrag.dropTarget = null;
      clearDropHighlight();
      setSidebarArmed(true);
    } else {
      valueDrag.overTrash = false;
      setSidebarArmed(false);
      const exclude = valueDrag.source.kind === 'existing' ? valueDrag.source.location : null;
      const target = findDropTarget(e.clientX, e.clientY, exclude);
      valueDrag.dropTarget = target;
      setDropHighlight(target);
    }
    return;
  }
  if (valueDragCandidate && e.pointerId === valueDragCandidate.pointerId) {
    const dx = e.clientX - valueDragCandidate.startX;
    const dy = e.clientY - valueDragCandidate.startY;
    if (Math.hypot(dx, dy) < 4) return;
    const candidate = valueDragCandidate;
    valueDragCandidate = null;
    startValueDrag(e, candidate);
  }
}

function onPointerUp(e: PointerEvent) {
  if (valueDragCandidate && valueDragCandidate.pointerId === e.pointerId) {
    const candidate = valueDragCandidate;
    valueDragCandidate = null;
    // Never crossed the drag threshold — a plain click. A real pointerup
    // (not a cancel) on an existing operator block samples-evaluates it; a
    // leaf, a palette prefab, or a cancelled gesture does nothing, same as
    // today.
    if (e.type === 'pointerup' && candidate.source.kind === 'existing' && candidate.source.value.kind === 'Op') {
      void previewClickedOperator(candidate.source.location, candidate.source.value);
    }
  }
  if (!valueDrag || valueDrag.pointerId !== e.pointerId) return;

  const finished = valueDrag;
  valueDrag = null;
  clearDropHighlight();
  setSidebarArmed(false);
  finished.ghostEl.remove();

  void (async () => {
    try {
      if (finished.overTrash) {
        if (finished.source.kind === 'existing') {
          const loc = finished.source.location;
          if (loc.kind === 'Floating' && loc.path.length === 0) {
            await removeFloatingValue(loc.floating_id);
          } else {
            await takeValue(loc); // discard — resets the source slot to a default
            unmarkCapsule(loc);
          }
        }
        // Fresh-from-sidebar dropped back on the sidebar: never created, nothing to undo.
        return;
      }
      if (finished.dropTarget) {
        const { location: targetLoc, el: targetEl } = finished.dropTarget;

        let incoming: ValueDto;
        if (finished.source.kind === 'fresh') {
          incoming = paletteValueFor(finished.source.valueKind);
        } else {
          incoming = await takeValue(finished.source.location);
          unmarkCapsule(finished.source.location);
        }

        // A floating card's root is just a positioned container holding one
        // value, not "a block someone placed there on purpose" the way an
        // operator embedded in a field is — it's the slot itself, there's no
        // separate outer thing to keep distinct from what's inside it. So
        // dropping onto one always swaps its content in place via put_value
        // (old content tucked away as the incoming operator's `saved`,
        // exactly like any other unboxed slot) rather than ejecting the old
        // value as a separate stray card next to it; the card keeps its
        // id/x/y throughout.
        const isFloatingRoot = targetLoc.kind === 'Floating' && targetLoc.path.length === 0;

        // A boxed *field* target (an operator, or a leaf capsule dropped in
        // earlier — see `boxed` in ValueBlock.vue) is a real block someone
        // placed there on purpose, so replacing it shouldn't just quietly
        // disappear it into the incoming operator's `saved` — take it out
        // and eject it as its own floating card next to the field first, so
        // it visibly reads as "moved aside" rather than deleted. An unboxed
        // target is just ordinary field content (typed in, not dragged in),
        // so it keeps the plain put_value-absorbs-the-old-value behavior.
        // Read the class off the live element rather than the value tree —
        // there's no frontend helper to resolve an arbitrary location to its
        // current ValueDto, and the rendered class already reflects exactly
        // the same `boxed` check we'd otherwise have to duplicate.
        if (!isFloatingRoot && targetEl.classList.contains('value-card-shape')) {
          const r = targetEl.getBoundingClientRect();
          const [x, y] = clientToCanvas(r.right + 16, r.top);
          const displaced = await takeValue(targetLoc);
          await putValue(targetLoc, incoming);
          markOrUnmark(targetLoc, incoming);
          await createFloatingValue(x, y, displaced);
        } else {
          await putValue(targetLoc, incoming);
          markOrUnmark(targetLoc, incoming);
        }
        return;
      }
      // Open canvas.
      const [x, y] = clientToCanvas(e.clientX - finished.offsetX, e.clientY - finished.offsetY);
      if (finished.source.kind === 'fresh') {
        await createFloatingValue(x, y, paletteValueFor(finished.source.valueKind));
      } else if (finished.source.location.kind === 'Floating' && finished.source.location.path.length === 0) {
        // Whole floating block, just repositioned — no content change. Apply
        // the new position locally first (same optimistic trick as
        // canvasDrag.ts's strand move) and reveal the real card immediately
        // after, so it reappears at the drop point instead of flashing at
        // its old position for the round trip to moveFloatingValue.
        const floatingId = finished.source.location.floating_id;
        const fv = state.current_macro?.floating_values?.find(f => f.id === floatingId);
        if (fv) {
          fv.x = x;
          fv.y = y;
        }
        if (finished.hiddenAnchorEl) finished.hiddenAnchorEl.style.visibility = '';
        await moveFloatingValue(floatingId, x, y);
      } else {
        const taken = await takeValue(finished.source.location);
        unmarkCapsule(finished.source.location);
        await createFloatingValue(x, y, taken);
      }
    } catch (err) {
      console.error('value drag drop failed:', err);
    } finally {
      // Cleared only once the backend call(s) above have resolved, so the
      // source slot/card never flashes back to its real pre-drag content in
      // the window between drop and the state-updated round trip landing.
      dragReveal.value = null;
      if (finished.hiddenAnchorEl) finished.hiddenAnchorEl.style.visibility = '';
    }
  })();
}

let listenersAttached = false;

/** Wires up the document-level pointer listeners. Idempotent. */
export function attachValueDragListeners() {
  if (listenersAttached) return;
  listenersAttached = true;
  document.addEventListener('pointermove', onPointerMove);
  document.addEventListener('pointerup', onPointerUp);
  document.addEventListener('pointercancel', onPointerUp);
}
