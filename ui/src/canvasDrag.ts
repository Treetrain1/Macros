// Imperative Scratch-style strand-canvas drag/drop/pan/zoom machinery —
// ported near-verbatim from the original vanilla app.js. This is deliberately
// NOT a reactive/declarative rewrite: it's a real-time pointer-event state
// machine doing live DOM measurement (getBoundingClientRect/offsetWidth) that
// Vue's reactivity graph has no visibility into, so a "more Vue" version
// would just re-derive the same imperative logic for no behavioral gain and
// real regression risk. See the per-function comments (carried over from the
// original) for the specific bugs this exact structure avoids.
import { state } from './store';
import { addInstruction, addStrand, mergeStrand, moveStrand, removeStrand, splitStrand } from './tauri';
import type { InstructionDto, MacroDto } from './types';

export const ROOT_ID = 'root';

// Strand x/y from the backend are canvas-space coordinates that can go
// negative; canvas-inner is sized to the strands' bounding box each render,
// offset by this padding so cards never touch the edge.
const CANVAS_PAD = 400;
// Bounds always extend at least this far past the origin in every direction,
// so there's room to pan around the root strand even when it's the only
// thing on the canvas.
const ROOT_MARGIN = 1000;

function cssEscape(s: string): string {
  return String(s).replace(/[^a-zA-Z0-9_-]/g, '\\$&');
}

function findStrand(strandId: string) {
  return state.current_macro?.strands?.find(st => st.id === strandId);
}

// ── Zoom (ctrl+scroll) ──────────────────────────────────────────────────────
// canvas-inner stays laid out at unscaled canvas-space coordinates and is
// visually scaled with a CSS transform; canvas-sizer's box is set to the
// zoomed footprint so the scrollbars/scroll range match what's on screen.
const MIN_ZOOM = 0.35;
const MAX_ZOOM = 2.5;
let canvasZoom = 1;
let currentMacroId: string | null = null;

// Bounds of the last render, needed to convert pointer/client coordinates
// back into canvas (strand.x/y) space when a drag ends on empty canvas.
let lastBounds = { minX: 0, minY: 0 };

/**
 * DOM geometry pass: measures the just-rendered (Vue-created) strand cards
 * and positions them in canvas space. Call this after Vue has patched the
 * DOM for state.current_macro.strands (e.g. from a `flush: 'post'` watcher).
 */
export function positionCanvas(macro: MacroDto | null | undefined) {
  const inner = document.getElementById('canvas-inner');
  const sizer = document.getElementById('canvas-sizer');
  const scrollEl = document.getElementById('canvas-scroll');
  if (!inner || !sizer || !scrollEl) return;

  const strands = macro?.strands ?? [];
  const macroId = macro?.id ?? null;
  if (macroId !== currentMacroId) canvasZoom = 1;

  const cardEls = new Map<string, HTMLElement>();
  for (const strand of strands) {
    const el = inner.querySelector<HTMLElement>(`.strand-card[data-strand-id="${cssEscape(strand.id)}"]`);
    if (el) cardEls.set(strand.id, el);
  }

  const prevMinX = lastBounds.minX;
  const prevMinY = lastBounds.minY;
  let minX = -ROOT_MARGIN, minY = -ROOT_MARGIN, maxX = ROOT_MARGIN, maxY = ROOT_MARGIN;
  for (const strand of strands) {
    const card = cardEls.get(strand.id);
    if (!card) continue;
    minX = Math.min(minX, strand.x);
    minY = Math.min(minY, strand.y);
    maxX = Math.max(maxX, strand.x + card.offsetWidth);
    maxY = Math.max(maxY, strand.y + card.offsetHeight);
  }
  lastBounds = { minX, minY };

  const innerW = maxX - minX + 2 * CANVAS_PAD;
  const innerH = maxY - minY + 2 * CANVAS_PAD;
  inner.style.width = `${innerW}px`;
  inner.style.height = `${innerH}px`;
  inner.style.transform = `scale(${canvasZoom})`;
  sizer.style.width = `${innerW * canvasZoom}px`;
  sizer.style.height = `${innerH * canvasZoom}px`;

  // A strand placed beyond the previous extent (e.g. a palette drop out in
  // blank space) pushes minX/minY outward, which shifts every card's
  // rendered position by the same amount. Without this, that shift happens
  // under a scroll position that didn't move, so the thing that was just
  // dropped visibly jumps away from the cursor — compensate scroll so the
  // content the user was looking at stays exactly where it was.
  if (minX !== prevMinX || minY !== prevMinY) {
    scrollEl.scrollLeft += (prevMinX - minX) * canvasZoom;
    scrollEl.scrollTop += (prevMinY - minY) * canvasZoom;
  }

  for (const strand of strands) {
    const card = cardEls.get(strand.id);
    if (!card) continue;
    card.style.left = `${strand.x - minX + CANVAS_PAD}px`;
    card.style.top = `${strand.y - minY + CANVAS_PAD}px`;
  }

  if (macroId !== currentMacroId) {
    currentMacroId = macroId;
    const root = strands.find(st => st.id === ROOT_ID);
    requestAnimationFrame(() => {
      if (root) {
        scrollEl.scrollLeft = Math.max(0, root.x - minX + CANVAS_PAD - 60);
        scrollEl.scrollTop = Math.max(0, root.y - minY + CANVAS_PAD - 60);
      } else {
        scrollEl.scrollLeft = 0;
        scrollEl.scrollTop = 0;
      }
    });
  }
}

function onCanvasWheel(e: WheelEvent) {
  if (!e.ctrlKey) return;
  e.preventDefault();
  const scrollEl = document.getElementById('canvas-scroll');
  if (!scrollEl) return;
  const rect = scrollEl.getBoundingClientRect();
  const canvasPtX = (e.clientX - rect.left + scrollEl.scrollLeft) / canvasZoom;
  const canvasPtY = (e.clientY - rect.top + scrollEl.scrollTop) / canvasZoom;

  const factor = e.deltaY < 0 ? 1.12 : 1 / 1.12;
  const newZoom = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, canvasZoom * factor));
  if (newZoom === canvasZoom) return;
  canvasZoom = newZoom;
  positionCanvas(state.current_macro);

  scrollEl.scrollLeft = canvasPtX * canvasZoom - (e.clientX - rect.left);
  scrollEl.scrollTop = canvasPtY * canvasZoom - (e.clientY - rect.top);
}

// ── Middle-click-drag panning ───────────────────────────────────────────────
interface PanState {
  pointerId: number;
  startX: number;
  startY: number;
  startScrollLeft: number;
  startScrollTop: number;
}
let pan: PanState | null = null;

function capturePointer(e: PointerEvent) {
  try {
    (e.target as Element | null)?.setPointerCapture?.(e.pointerId);
  } catch (err) {
    console.error('setPointerCapture failed:', err);
  }
}

function beginPan(e: PointerEvent) {
  if (e.button !== 1) return;
  e.preventDefault();
  capturePointer(e);
  const scrollEl = document.getElementById('canvas-scroll');
  if (!scrollEl) return;
  pan = {
    pointerId: e.pointerId,
    startX: e.clientX,
    startY: e.clientY,
    startScrollLeft: scrollEl.scrollLeft,
    startScrollTop: scrollEl.scrollTop,
  };
  scrollEl.classList.add('panning');
}

// ── Instruction sidebar (palette + trash) ───────────────────────────────────

function isOverSidebar(e: PointerEvent): boolean {
  const el = document.getElementById('instruction-sidebar');
  if (!el) return false;
  const rect = el.getBoundingClientRect();
  return e.clientX >= rect.left && e.clientX <= rect.right && e.clientY >= rect.top && e.clientY <= rect.bottom;
}

function setSidebarArmed(armed: boolean) {
  document.getElementById('instruction-sidebar')?.classList.toggle('trash-armed', armed);
}

// ── Drag pickup (grip / whole-strand) ───────────────────────────────────────
interface DragCandidate {
  strandId: string;
  index: number;
  pointerId: number;
  startX: number;
  startY: number;
}
let dragCandidate: DragCandidate | null = null;

interface DragState {
  pointerId: number;
  offsetX: number;
  offsetY: number;
  ghostEl: HTMLElement;
  resolvedId: string | null;
  resolvingPromise: Promise<string | void> | null;
  snap: { targetId: string; index: number } | null;
  overTrash?: boolean;
  // Real DOM nodes hidden (visibility: hidden) while the ghost stands in for
  // them. Vue reuses these exact nodes by key on the next patch and never
  // touches this inline style itself, so it MUST be restored explicitly on
  // every drag end (move/merge/trash/error) or the block stays invisible
  // forever — only a full remount (undo/redo, reload) would ever clear it.
  hiddenCardEl: HTMLElement | null;
  hiddenRowEls: HTMLElement[];
}
let drag: DragState | null = null;

interface PaletteDragState {
  pointerId: number;
  insType: InstructionDto['type'];
  offsetX: number;
  offsetY: number;
  ghostEl: HTMLElement;
  snap: { targetId: string; index: number } | null;
}
let paletteDrag: PaletteDragState | null = null;

export function defaultInstruction(type: InstructionDto['type']): InstructionDto {
  switch (type) {
    case 'Wait': return { type: 'Wait', duration: 1000, randomness: 0 };
    case 'Text': return { type: 'Text', text: 'text' };
    case 'Key': return { type: 'Key', key: 'a', direction: 'Click' };
    case 'Button': return { type: 'Button', button: 'Left', direction: 'Click' };
    case 'MoveMouse': return { type: 'MoveMouse', x: 0, y: 0, coordinate: 'Relative' };
    case 'Scroll': return { type: 'Scroll', amount: 4, axis: 'Vertical' };
    case 'Command': return { type: 'Command', command: '' };
    case 'Comment': return { type: 'Comment', comment: '' };
    default: return { type: 'Comment', comment: '' };
  }
}

export function beginPickup(e: PointerEvent, strandId: string, index: number) {
  if (state.recording_phase.phase === 'Active') return;
  if (e.button !== undefined && e.button !== 0) return;
  e.preventDefault();
  capturePointer(e);
  dragCandidate = { strandId, index, pointerId: e.pointerId, startX: e.clientX, startY: e.clientY };
}

// Dragging a palette entry previews the actual instruction block it'll
// create (not the sidebar chip) — dropped on empty canvas it becomes exactly
// that: a single ordinary-looking block, nothing else.
export function beginPaletteDrag(e: PointerEvent, insType: InstructionDto['type'], ghostRowEl: HTMLElement) {
  if (state.recording_phase.phase === 'Active') return;
  if (e.button !== undefined && e.button !== 0) return;
  e.preventDefault();
  capturePointer(e);

  const ghost = document.createElement('div');
  ghost.className = 'strand-drag-ghost';
  const ghostCard = document.createElement('div');
  ghostCard.className = 'strand-card';
  const ghostBody = document.createElement('div');
  ghostBody.className = 'strand-body';
  ghostBody.appendChild(ghostRowEl);
  ghostCard.appendChild(ghostBody);
  ghost.appendChild(ghostCard);
  document.body.appendChild(ghost);

  paletteDrag = {
    pointerId: e.pointerId,
    insType,
    offsetX: 14,
    offsetY: 14,
    ghostEl: ghost,
    snap: null,
  };
  positionGhost(e);
}

let ghostRafPending = false;
let lastPointerEvent: PointerEvent | null = null;
function positionGhost(e: PointerEvent) {
  lastPointerEvent = e;
  if (ghostRafPending) return;
  ghostRafPending = true;
  requestAnimationFrame(() => {
    ghostRafPending = false;
    const active = drag ?? paletteDrag;
    if (!active || !lastPointerEvent) return;
    active.ghostEl.style.transform = `translate(${lastPointerEvent.clientX - active.offsetX}px, ${lastPointerEvent.clientY - active.offsetY}px)`;
  });
}

let snapIndicatorEl: HTMLElement | null = null;
function clearSnapIndicator() {
  if (snapIndicatorEl) {
    snapIndicatorEl.remove();
    snapIndicatorEl = null;
  }
}

const SNAP_THRESHOLD = 28;

// Shared by both strand-drags (snapping an existing block elsewhere) and
// palette-drags (dropping a brand new instruction onto a strand); writes the
// result onto `target.snap` and updates the shared snap-line indicator.
interface SnapCandidate { targetId: string; index: number; dist: number; y: number; left: number; width: number }

function updateSnapTarget(e: PointerEvent, target: { snap: { targetId: string; index: number } | null }, excludeId: string | null | undefined) {
  const cards = Array.from(document.querySelectorAll<HTMLElement>('.strand-card'));
  let best: SnapCandidate | null = null;
  for (const card of cards) {
    const id = card.dataset.strandId;
    if (!id || id === excludeId) continue;
    const cardRect = card.getBoundingClientRect();
    if (e.clientX < cardRect.left - 60 || e.clientX > cardRect.right + 60) continue;
    const body = card.querySelector('.strand-body');
    const rows = Array.from(card.querySelectorAll('.instruction-row'));
    const boundaries = rows.map(r => r.getBoundingClientRect().top);
    boundaries.push(rows.length ? rows[rows.length - 1].getBoundingClientRect().bottom : (body?.getBoundingClientRect().top ?? cardRect.top) + 8);
    for (let idx = 0; idx < boundaries.length; idx++) {
      const y = boundaries[idx];
      const dist = Math.abs(e.clientY - y);
      if (dist <= SNAP_THRESHOLD && (best === null || dist < (best as SnapCandidate).dist)) {
        best = { targetId: id, index: idx, dist, y, left: cardRect.left, width: cardRect.width };
      }
    }
  }
  target.snap = best !== null ? { targetId: (best as SnapCandidate).targetId, index: (best as SnapCandidate).index } : null;

  if (best !== null) {
    const b = best as SnapCandidate;
    if (!snapIndicatorEl) {
      snapIndicatorEl = document.createElement('div');
      snapIndicatorEl.className = 'strand-snap-indicator';
      document.body.appendChild(snapIndicatorEl);
    }
    snapIndicatorEl.style.left = `${b.left}px`;
    snapIndicatorEl.style.top = `${b.y - 2}px`;
    snapIndicatorEl.style.width = `${b.width}px`;
  } else {
    clearSnapIndicator();
  }
}

function clientToCanvas(clientX: number, clientY: number): [number, number] {
  const inner = document.getElementById('canvas-inner');
  if (!inner) return [0, 0];
  const rect = inner.getBoundingClientRect();
  return [
    Math.round((clientX - rect.left) / canvasZoom - CANVAS_PAD + lastBounds.minX),
    Math.round((clientY - rect.top) / canvasZoom - CANVAS_PAD + lastBounds.minY),
  ];
}

// The ghost is built from real `.strand-card`/`.instruction-row` clones (not
// a specially-styled wrapper), so a block being dragged looks exactly like
// it does at rest — the block is the strand, dragging just moves it.
function startDrag(e: PointerEvent, candidate: DragCandidate) {
  const { strandId, index, pointerId } = candidate;
  const strand = findStrand(strandId);
  if (!strand) return;

  const cardEl = document.querySelector<HTMLElement>(`.strand-card[data-strand-id="${cssEscape(strandId)}"]`);
  const wholeStrandGrab = strandId !== ROOT_ID && index === 0;

  const ghost = document.createElement('div');
  ghost.className = 'strand-drag-ghost';
  let anchorRect: { left: number; top: number } = cardEl ? cardEl.getBoundingClientRect() : { left: e.clientX, top: e.clientY };
  let hiddenCardEl: HTMLElement | null = null;
  let hiddenRowEls: HTMLElement[] = [];

  if (wholeStrandGrab) {
    if (cardEl) {
      ghost.appendChild(cardEl.cloneNode(true) as HTMLElement);
      cardEl.style.visibility = 'hidden';
      hiddenCardEl = cardEl;
    }
  } else {
    const rowEls = cardEl ? Array.from(cardEl.querySelectorAll<HTMLElement>('.instruction-row')).slice(index) : [];
    if (rowEls[0]) anchorRect = rowEls[0].getBoundingClientRect();
    const ghostCard = document.createElement('div');
    ghostCard.className = 'strand-card';
    const ghostBody = document.createElement('div');
    ghostBody.className = 'strand-body';
    rowEls.forEach(el => {
      el.style.visibility = 'hidden';
      ghostBody.appendChild(el.cloneNode(true) as HTMLElement);
    });
    ghostCard.appendChild(ghostBody);
    ghost.appendChild(ghostCard);
    hiddenRowEls = rowEls;
  }
  document.body.appendChild(ghost);

  const newDrag: DragState = {
    pointerId,
    offsetX: e.clientX - anchorRect.left,
    offsetY: e.clientY - anchorRect.top,
    ghostEl: ghost,
    resolvedId: null,
    resolvingPromise: null,
    snap: null,
    hiddenCardEl,
    hiddenRowEls,
  };
  drag = newDrag;

  if (wholeStrandGrab) {
    newDrag.resolvedId = strandId;
  } else {
    newDrag.resolvingPromise = splitStrand(strandId, index, strand.x + 24, strand.y + 24)
      .then(newId => {
        newDrag.resolvedId = newId;
        return newId;
      })
      .catch(err => {
        console.error('split_strand failed:', err);
      });
  }

  positionGhost(e);
}

// Each branch below only ever handles the event once it has confirmed the
// event's pointerId actually belongs to that gesture — never on a bare "some
// other gesture is active" check. Only one of pan/paletteDrag/drag/
// dragCandidate is ever really in flight at once, but if any one of them were
// ever left stuck (e.g. a pointerup missed while panning), a blanket
// early-return here would silently swallow every *other* pointer's
// moves/ups forever, which is exactly what made palette drops stop working
// in the original implementation.
function onPointerMove(e: PointerEvent) {
  if (pan && e.pointerId === pan.pointerId) {
    const scrollEl = document.getElementById('canvas-scroll');
    if (scrollEl) {
      scrollEl.scrollLeft = pan.startScrollLeft - (e.clientX - pan.startX);
      scrollEl.scrollTop = pan.startScrollTop - (e.clientY - pan.startY);
    }
    return;
  }
  if (paletteDrag && e.pointerId === paletteDrag.pointerId) {
    positionGhost(e);
    if (isOverSidebar(e)) {
      paletteDrag.snap = null;
      clearSnapIndicator();
    } else {
      updateSnapTarget(e, paletteDrag, null);
    }
    return;
  }
  if (drag && e.pointerId === drag.pointerId) {
    positionGhost(e);
    if (isOverSidebar(e)) {
      drag.overTrash = true;
      drag.snap = null;
      clearSnapIndicator();
      setSidebarArmed(true);
    } else {
      drag.overTrash = false;
      setSidebarArmed(false);
      updateSnapTarget(e, drag, drag.resolvedId ?? dragCandidate?.strandId);
    }
    return;
  }
  if (dragCandidate && e.pointerId === dragCandidate.pointerId) {
    const dx = e.clientX - dragCandidate.startX;
    const dy = e.clientY - dragCandidate.startY;
    if (Math.hypot(dx, dy) < 4) return;
    const candidate = dragCandidate;
    dragCandidate = null;
    startDrag(e, candidate);
  }
}

function onPointerUp(e: PointerEvent) {
  if (pan && pan.pointerId === e.pointerId) {
    pan = null;
    document.getElementById('canvas-scroll')?.classList.remove('panning');
  }
  if (dragCandidate && dragCandidate.pointerId === e.pointerId) {
    dragCandidate = null;
  }

  if (paletteDrag && paletteDrag.pointerId === e.pointerId) {
    const finished = paletteDrag;
    paletteDrag = null;
    clearSnapIndicator();
    finished.ghostEl.remove();

    if (!isOverSidebar(e)) {
      const ins = defaultInstruction(finished.insType);
      void (async () => {
        try {
          if (finished.snap) {
            await addInstruction(finished.snap.targetId, finished.snap.index, ins);
          } else {
            const [x, y] = clientToCanvas(e.clientX - finished.offsetX, e.clientY - finished.offsetY);
            await addStrand(x, y, ins);
          }
        } catch (err) {
          console.error('palette drop failed:', err);
        }
      })();
    }
    return;
  }

  if (!drag || drag.pointerId !== e.pointerId) return;

  const finished = drag;
  drag = null;
  clearSnapIndicator();
  setSidebarArmed(false);
  finished.ghostEl.remove();
  // Restore unconditionally (move/merge/trash/error) — the backend round
  // trip that would otherwise reveal these nodes again is async and, for
  // trash/merge, may never touch them at all.
  if (finished.hiddenCardEl) finished.hiddenCardEl.style.visibility = '';
  for (const el of finished.hiddenRowEls) el.style.visibility = '';

  void (async () => {
    const id = finished.resolvedId ?? (finished.resolvingPromise ? await finished.resolvingPromise : null);
    if (!id) return;
    if (finished.overTrash) {
      await removeStrand(id);
    } else if (finished.snap) {
      await mergeStrand(id, finished.snap.targetId, finished.snap.index);
    } else {
      const [x, y] = clientToCanvas(e.clientX - finished.offsetX, e.clientY - finished.offsetY);
      // Optimistic: apply locally so the card renders at the drop point
      // immediately instead of sitting at its old position (or hidden, pre-
      // fix) until the invoke + state-updated round trip lands — that lag
      // is what read as the block "teleporting" once the mouse was let go.
      const strand = findStrand(id);
      if (strand) {
        strand.x = x;
        strand.y = y;
      }
      await moveStrand(id, x, y);
    }
  })();
}

let listenersAttached = false;

/** Wires up the document/canvas-scroll-level pointer listeners. Idempotent. */
export function attachDragListeners() {
  if (listenersAttached) return;
  listenersAttached = true;
  document.getElementById('canvas-scroll')?.addEventListener('pointerdown', beginPan);
  document.addEventListener('pointermove', onPointerMove);
  document.addEventListener('pointerup', onPointerUp);
  document.addEventListener('pointercancel', onPointerUp);
  document.getElementById('canvas-scroll')?.addEventListener('wheel', onCanvasWheel, { passive: false });
}
