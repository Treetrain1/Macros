// Pointer-drag state machine for value blocks (Number/Text/Add/Sub/Mul/Div),
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
import { state } from './store';
import { capturePointer, clientToCanvas, isOverSidebar, setSidebarArmed } from './canvasDrag';
import { createFloatingValue, moveFloatingValue, putValue, removeFloatingValue, setValueKind, takeValue } from './tauri';
import type { ValueDto, ValueKind, ValueLocationDto } from './types';
import { defaultValueForKind } from './types';

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
  // For an 'existing' pickup, the real on-screen block being dragged — hidden
  // (not moved/cloned away) for the duration so the field it came from reads
  // as empty immediately, instead of only updating once the drop's async
  // take/put round trip lands. Restored on pointerup regardless of outcome;
  // Vue's own re-render (once the backend call resolves) settles the rest,
  // same idea as canvasDrag.ts's hiddenRowEls.
  hiddenSourceEl: HTMLElement | null;
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
    valueDrag.ghostEl.style.transform = `translate(${tx}px, ${ty}px)`;
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

  let hiddenSourceEl: HTMLElement | null = null;
  if (candidate.source.kind === 'existing') {
    hiddenSourceEl = candidate.anchorEl;
    hiddenSourceEl.style.visibility = 'hidden';
  }

  valueDrag = {
    pointerId: candidate.pointerId,
    offsetX,
    offsetY,
    ghostEl: ghost,
    source: candidate.source,
    dropTarget: null,
    overTrash: false,
    hiddenSourceEl,
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
 * `[data-value-location]` block, skipping the dragged block's own subtree
 * (can't drop a block into itself or one of its own operands). */
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
    if (!exclude || !isSelfOrDescendant(location, exclude)) {
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
    valueDragCandidate = null;
  }
  if (!valueDrag || valueDrag.pointerId !== e.pointerId) return;

  const finished = valueDrag;
  valueDrag = null;
  clearDropHighlight();
  setSidebarArmed(false);
  finished.ghostEl.remove();
  if (finished.hiddenSourceEl) finished.hiddenSourceEl.style.visibility = '';

  void (async () => {
    try {
      if (finished.overTrash) {
        if (finished.source.kind === 'existing') {
          const loc = finished.source.location;
          if (loc.kind === 'Floating' && loc.path.length === 0) {
            await removeFloatingValue(loc.floating_id);
          } else {
            await takeValue(loc); // discard — resets the source slot to a default
          }
        }
        // Fresh-from-sidebar dropped back on the sidebar: never created, nothing to undo.
        return;
      }
      if (finished.dropTarget) {
        if (finished.source.kind === 'fresh') {
          await setValueKind(finished.dropTarget.location, finished.source.valueKind);
        } else {
          const taken = await takeValue(finished.source.location);
          await putValue(finished.dropTarget.location, taken);
        }
        return;
      }
      // Open canvas.
      const [x, y] = clientToCanvas(e.clientX - finished.offsetX, e.clientY - finished.offsetY);
      if (finished.source.kind === 'fresh') {
        await createFloatingValue(x, y, defaultValueForKind(finished.source.valueKind));
      } else if (finished.source.location.kind === 'Floating' && finished.source.location.path.length === 0) {
        // Whole floating block, just repositioned — no content change.
        await moveFloatingValue(finished.source.location.floating_id, x, y);
      } else {
        const taken = await takeValue(finished.source.location);
        await createFloatingValue(x, y, taken);
      }
    } catch (err) {
      console.error('value drag drop failed:', err);
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
