// Pointer-drag state machine for value blocks (Number/Text/Add/Sub/Mul/Div/Random).
// Kept separate from canvasDrag.ts's strand drag/snap machinery so a value
// block structurally can't attach to a strand's instruction list (this file
// never imports addInstruction/addStrand/mergeStrand/splitStrand).
//
// Same imperative pointer-tracking/hit-testing approach as canvasDrag.ts, but
// simpler: existing blocks always drag as a cloned ghost, and take/put/create
// only fire at drop time, not pickup.
import { ref } from 'vue';
import { state } from './store';
import { capturePointer, clientToCanvas, getCanvasZoom, isOverSidebar, setSidebarArmed } from './canvasDrag';
import { createFloatingValue, moveFloatingValue, previewValue, putValue, removeFloatingValue, takeValue } from './tauri';
import { paletteValueFor } from './paletteState';
import { paletteCallValueFor } from './blockDefs';
import { numberValue } from './types';
import { locationsEqual } from './invalidField';
import type { ValueDto, ValueKind, ValueLocationDto } from './types';

/** The `ValueDto` a fresh sidebar prefab represents, dispatched by kind —
 * `Call:<blockId>` goes to blockDefs.ts, everything else to
 * paletteState.ts's `paletteValueFor`. */
function resolveFreshValue(kind: ValueKind): ValueDto {
  return kind.startsWith('Call:') ? paletteCallValueFor(kind.slice('Call:'.length)) : paletteValueFor(kind);
}

// Field/subfield locations whose current leaf (Number/Text) got there by being
// dropped in (not just typed) — ValueBlock.vue's `boxed` consults this so a
// dropped-in leaf keeps its capsule look and stays pickup-draggable. Frontend-
// only affordance, not persisted; doesn't survive reload or undo/redo.
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

// While a field/subfield value is mid-drag, its origin ValueBlock renders this
// instead of its real prop — the value take_value would leave behind — so the
// slot reads correctly immediately instead of sitting blank until the async
// take/put round trip lands.
export const dragReveal = ref<{ location: ValueLocationDto; value: ValueDto } | null>(null);

const EVAL_PREVIEW_TIMEOUT_MS = 2500;

// A click (pointerdown+up without crossing the drag threshold) on an operator
// block samples-evaluates it and shows the result here; ValueBlock.vue renders
// a tooltip beside the matching location. Auto-dismisses after a timeout.
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

// Same idea as `evalPreview`, but for a sidebar prefab — keyed by `kind`
// since it has no `ValueLocationDto` yet. PaletteValueBlock.vue renders it.
export const paletteEvalPreview = ref<{ kind: ValueKind; text: string; error: boolean } | null>(null);
let paletteEvalPreviewTimer: ReturnType<typeof setTimeout> | null = null;

function showPaletteEvalPreview(kind: ValueKind, text: string, error: boolean) {
  if (paletteEvalPreviewTimer) clearTimeout(paletteEvalPreviewTimer);
  paletteEvalPreview.value = { kind, text, error };
  paletteEvalPreviewTimer = setTimeout(() => {
    paletteEvalPreview.value = null;
    paletteEvalPreviewTimer = null;
  }, EVAL_PREVIEW_TIMEOUT_MS);
}

function clearPaletteEvalPreview() {
  if (paletteEvalPreviewTimer) {
    clearTimeout(paletteEvalPreviewTimer);
    paletteEvalPreviewTimer = null;
  }
  paletteEvalPreview.value = null;
}

// Number/Text prefabs are plain literals — previewing "5" as "5" isn't useful,
// so only operators/variables preview on click.
function isPreviewableKind(kind: ValueKind): boolean {
  return kind !== 'Number' && kind !== 'Text';
}

async function previewClickedPaletteValue(kind: ValueKind) {
  try {
    const text = await previewValue(resolveFreshValue(kind));
    showPaletteEvalPreview(kind, text, false);
  } catch (err) {
    showPaletteEvalPreview(kind, String(err), true);
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
  // The real floating-card element, hidden for the drag's duration so it
  // doesn't sit visible next to the ghost tracking the pointer.
  hiddenAnchorEl: HTMLElement | null;
}
let valueDrag: ValueDragState | null = null;

/** Picking up a block that already exists (embedded in a field/subfield, or
 * floating on canvas). `anchorEl` is cloned into the drag ghost once the
 * pointer crosses the click-vs-drag threshold. */
export function beginValuePickup(e: PointerEvent, location: ValueLocationDto, value: ValueDto, anchorEl: HTMLElement) {
  if (state.recording_phase.phase === 'Active') return;
  if (e.button !== undefined && e.button !== 0) return;
  e.preventDefault();
  capturePointer(e);
  clearEvalPreview();
  clearPaletteEvalPreview();
  valueDragCandidate = { pointerId: e.pointerId, startX: e.clientX, startY: e.clientY, anchorEl, source: { kind: 'existing', location, value } };
}

/** Dragging a brand-new block off the sidebar's "Operator" section.
 * `anchorEl` is that kind's hidden ghost template. */
export function beginValuePaletteDrag(e: PointerEvent, valueKind: ValueKind, anchorEl: HTMLElement) {
  if (state.recording_phase.phase === 'Active') return;
  if (e.button !== undefined && e.button !== 0) return;
  e.preventDefault();
  capturePointer(e);
  clearEvalPreview();
  clearPaletteEvalPreview();
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

  // A palette drag's anchorEl is the sidebar's hidden off-screen template, so
  // its rect is off-screen, not cursor-relative — center the ghost under the
  // pointer instead of deriving a click-relative offset from it.
  const isFresh = candidate.source.kind === 'fresh';
  const offsetX = isFresh ? rect.width / 2 : e.clientX - rect.left;
  const offsetY = isFresh ? rect.height / 2 : e.clientY - rect.top;

  let hiddenAnchorEl: HTMLElement | null = null;
  if (candidate.source.kind === 'existing') {
    const { location, value } = candidate.source;
    // A whole floating block has no field to reveal a placeholder in — hide
    // the real card (already cloned into the ghost) instead.
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

/** Walks up to the nearest `[data-value-location]` block, skipping the
 * dragged block's own subtree (can't drop into itself or its own operand). */
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
    // Never crossed the drag threshold — a plain click. A real pointerup (not
    // a cancel) on an operator/variable block or prefab samples-evaluates it.
    if (e.type === 'pointerup') {
      if (candidate.source.kind === 'existing' && (candidate.source.value.kind === 'Op' || candidate.source.value.kind === 'Var')) {
        void previewClickedOperator(candidate.source.location, candidate.source.value);
      } else if (candidate.source.kind === 'fresh' && isPreviewableKind(candidate.source.valueKind)) {
        void previewClickedPaletteValue(candidate.source.valueKind);
      }
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
          incoming = resolveFreshValue(finished.source.valueKind);
        } else {
          incoming = await takeValue(finished.source.location);
          unmarkCapsule(finished.source.location);
        }

        // A floating card's root is just the slot itself (not "a block placed
        // on purpose"), so dropping onto it always swaps content in place via
        // put_value, keeping the card's id/x/y.
        const isFloatingRoot = targetLoc.kind === 'Floating' && targetLoc.path.length === 0;

        // A boxed field target (operator, or a dropped-in leaf capsule) was
        // placed there on purpose, so replacing it ejects the old value as its
        // own floating card instead of silently absorbing it. An unboxed
        // (typed-in) target keeps plain put_value behavior. Read the class off
        // the live element since it already reflects ValueBlock.vue's `boxed`.
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
        await createFloatingValue(x, y, resolveFreshValue(finished.source.valueKind));
      } else if (finished.source.location.kind === 'Floating' && finished.source.location.path.length === 0) {
        // Whole floating block, just repositioned. Apply the position
        // optimistically and reveal the real card immediately (same trick as
        // canvasDrag.ts's strand move), rather than waiting on moveFloatingValue.
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
      // Cleared only after the backend call(s) resolve, so the source
      // slot/card never flashes back to its pre-drag content in between.
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
