// Imperative Scratch-style strand-canvas drag/drop/pan/zoom machinery —
// ported near-verbatim from the original vanilla app.js. This is deliberately
// NOT a reactive/declarative rewrite: it's a real-time pointer-event state
// machine doing live DOM measurement (getBoundingClientRect/offsetWidth) that
// Vue's reactivity graph has no visibility into, so a "more Vue" version
// would just re-derive the same imperative logic for no behavioral gain and
// real regression risk. See the per-function comments (carried over from the
// original) for the specific bugs this exact structure avoids.
import { state } from './store';
import { addInstruction, addStrand, deleteBlock, mergeStrand, moveStrand, removeStrand, splitStrand } from './tauri';
import { clonePaletteInstruction } from './paletteState';
import { paletteCallInstructionFor } from './blockDefs';
import type { InstructionDto, MacroDto } from './types';
import { isCapType, isHeaderType } from './types';

// Strand x/y from the backend are canvas-space coordinates that can go
// negative; canvas-inner is sized to the strands' bounding box each render,
// offset by this padding so cards never touch the edge.
const CANVAS_PAD = 400;
// Bounds always extend at least this far past the canvas origin in every
// direction, so there's room to pan around even when a macro has only one
// strand sitting right at (0, 0).
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

// Value-block drag ghosts (valueDrag.ts) live outside canvas-inner's scaled
// subtree too, so they need this to counter-scale themselves the same way
// strand/palette ghosts do below.
export function getCanvasZoom(): number {
  return canvasZoom;
}

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
  const floatingValues = macro?.floating_values ?? [];
  const macroId = macro?.id ?? null;
  if (macroId !== currentMacroId) canvasZoom = 1;

  const cardEls = new Map<string, HTMLElement>();
  for (const strand of strands) {
    const el = inner.querySelector<HTMLElement>(`.strand-card[data-strand-id="${cssEscape(strand.id)}"]`);
    if (el) cardEls.set(strand.id, el);
  }
  const floatingCardEls = new Map<string, HTMLElement>();
  for (const fv of floatingValues) {
    const el = inner.querySelector<HTMLElement>(`.value-floating-card[data-floating-id="${cssEscape(fv.id)}"]`);
    if (el) floatingCardEls.set(fv.id, el);
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
  for (const fv of floatingValues) {
    const card = floatingCardEls.get(fv.id);
    if (!card) continue;
    minX = Math.min(minX, fv.x);
    minY = Math.min(minY, fv.y);
    maxX = Math.max(maxX, fv.x + card.offsetWidth);
    maxY = Math.max(maxY, fv.y + card.offsetHeight);
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
  for (const fv of floatingValues) {
    const card = floatingCardEls.get(fv.id);
    if (!card) continue;
    card.style.left = `${fv.x - minX + CANVAS_PAD}px`;
    card.style.top = `${fv.y - minY + CANVAS_PAD}px`;
  }

  // A split-drag's new strand renders here as a real, fully visible card at
  // the split point the instant its state update lands — which can be well
  // before the user drops anywhere. Left alone, that reads as "the block
  // isn't following the cursor": a separate ghost clone quietly tracks the
  // pointer while this solid, static card just sits there until drop calls
  // moveStrand. Hide it for the duration, same idea as the whole-strand-grab
  // case where the real card itself becomes the ghost.
  //
  // The resolvedId is set asynchronously (when the splitStrand invoke
  // resolves), but the backend can emit state-updated before that response
  // arrives — so we also check preExistingStrandIds to catch new strands
  // even before resolvedId is known.
  if (drag && !drag.restoreCard) {
    const targetId = drag.resolvedId;
    if (targetId) {
      const newCard = cardEls.get(targetId);
      if (newCard && newCard !== drag.hiddenNewCardEl) {
        newCard.style.visibility = 'hidden';
        drag.hiddenNewCardEl = newCard;
      }
    } else {
      // resolvedId not yet available — hide any strand that wasn't there
      // before the drag started (the newly created split strand).
      for (const [id, card] of cardEls) {
        if (!drag.preExistingStrandIds.has(id) && card !== drag.hiddenNewCardEl) {
          card.style.visibility = 'hidden';
          drag.hiddenNewCardEl = card;
        }
      }
    }
  }

  if (macroId !== currentMacroId) {
    currentMacroId = macroId;
    requestAnimationFrame(() => {
      // Center the initial view on the canvas origin — strands generally
      // spawn near (0, 0), and there's no longer a single canonical strand
      // to scroll to.
      scrollEl.scrollLeft = Math.max(0, -minX + CANVAS_PAD - 60);
      scrollEl.scrollTop = Math.max(0, -minY + CANVAS_PAD - 60);
    });
  }
}

function onCanvasWheel(e: WheelEvent) {
  if (!e.ctrlKey) return;
  if (!(e.target as Element)?.closest?.('#canvas-scroll')) return;
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

export function capturePointer(e: PointerEvent) {
  try {
    (e.target as Element | null)?.setPointerCapture?.(e.pointerId);
  } catch (err) {
    console.error('setPointerCapture failed:', err);
  }
}

// On Linux, a middle click is X11's "paste primary selection" gesture —
// CEF honors it against whatever element already has focus, independent of
// hit-testing the click point, so preventDefault()-ing the pointerdown above
// doesn't stop it. The paste still goes through the normal cancelable
// ClipboardEvent though, so swallow that instead while a middle-click pan is
// in flight (a plain click that never reaches beginPan's checks never sets
// this, so real Ctrl+V/menu pastes elsewhere are untouched).
let blockPrimaryPaste = false;
let blockPrimaryPasteGeneration = 0;

function blockMiddleClickPaste(e: ClipboardEvent) {
  if (!blockPrimaryPaste) return;
  e.preventDefault();
  e.stopImmediatePropagation();
}

function beginPan(e: PointerEvent) {
  if (e.button !== 1) return;
  if (!(e.target as Element)?.closest?.('#canvas-scroll')) return;
  blockPrimaryPaste = true;
  blockPrimaryPasteGeneration++;
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

export function isOverSidebar(e: PointerEvent): boolean {
  const el = document.getElementById('instruction-sidebar');
  if (!el) return false;
  const rect = el.getBoundingClientRect();
  return e.clientX >= rect.left && e.clientX <= rect.right && e.clientY >= rect.top && e.clientY <= rect.bottom;
}

export function setSidebarArmed(armed: boolean) {
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
  // Set for a whole-strand grab of a strand headed by a `BlockHeader` — its
  // custom block's id, so dropping this on the trash deletes the block
  // definition (and every call site referencing it, see
  // `Macro::remove_block`) via `deleteBlock` instead of just detaching the
  // now-orphaned body strand via `removeStrand`, which would leave the
  // block's prefab still sitting in the "My Blocks" sidebar section with no
  // body behind it.
  blockId: string | null;
  // Whole-strand grab of a strand headed by a "When Ran" block: it can be
  // moved around freely but never snapped/merged into another strand (that
  // would attach it underneath something), so snap detection is skipped
  // entirely for the whole drag.
  noSnap: boolean;
  // The real DOM node(s) being dragged are physically relocated into the
  // ghost (not cloned/hidden), so the strand itself visibly follows the
  // pointer. Vue reuses these exact nodes by key on the next patch and never
  // touches their position in the tree itself, so they MUST be moved back to
  // where they came from explicitly on every drag end (move/merge/trash/
  // error) or Vue's next patch gets confused about where they live — only a
  // full remount (undo/redo, reload) would otherwise reset them.
  restoreCard: { el: HTMLElement; parent: Node; next: Node | null } | null;
  // Partial (split) drags can't use the same real-node move: splitStrand
  // fires at pickup and its response — which makes Vue patch the old
  // strand's row list — can land mid-drag, while these rows are sitting
  // detached inside the ghost. Vue would then unmount them from the wrong
  // (ghost) parent, and restoring them afterwards would resurrect an
  // orphaned zombie node Vue no longer tracks. So these stay hidden clones
  // instead, same as before the real-node-move change.
  hiddenRowEls: HTMLElement[];
  // Once a split resolves, its brand-new strand renders as a real, fully
  // visible card at the split point almost immediately (the backend event
  // round trip is near-instant) — long before drop. positionCanvas hides it
  // for us each render (see there); tracked here purely so pointerup knows
  // what to unhide.
  hiddenNewCardEl: HTMLElement | null;
  // Strand IDs that existed when the drag started. Used by positionCanvas to
  // detect newly created split-strand cards even before the invoke response
  // sets resolvedId — the backend's state-updated event can arrive before the
  // invoke resolves, leaving a window where the new card renders unhidden.
  preExistingStrandIds: Set<string>;
}
let drag: DragState | null = null;

interface PaletteDragState {
  pointerId: number;
  insType: InstructionDto['type'];
  // Set only for a `CallBlock` drag from the "My Blocks" section — which
  // specific custom block, since (unlike every other instruction type)
  // there's no single fixed prefab: `clonePaletteInstruction` can't resolve
  // it from `insType` alone. See `beginPaletteDrag`'s `blockId` param.
  blockId?: string;
  offsetX: number;
  offsetY: number;
  ghostEl: HTMLElement;
  snap: { targetId: string; index: number } | null;
}
let paletteDrag: PaletteDragState | null = null;

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
export function beginPaletteDrag(e: PointerEvent, insType: InstructionDto['type'], ghostRowEl: HTMLElement, blockId?: string) {
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
    blockId,
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
    const tx = lastPointerEvent.clientX - active.offsetX;
    const ty = lastPointerEvent.clientY - active.offsetY;
    active.ghostEl.style.transform = `translate(${tx}px, ${ty}px) scale(${canvasZoom})`;
    active.ghostEl.style.transformOrigin = '0 0';
  });
}

const SNAP_THRESHOLD = 28;

// Shared by both strand-drags (snapping an existing block elsewhere) and
// palette-drags (dropping a brand new instruction onto a strand); writes the
// result onto `target.snap` and updates the shared snap preview.
interface SnapCandidate { targetId: string; index: number; dist: number; y: number; left: number; width: number }

function updateSnapTarget(e: PointerEvent, target: { snap: { targetId: string; index: number } | null }, excludeId: string | null | undefined, ghostEl?: HTMLElement) {
  const ghostRect = ghostEl?.getBoundingClientRect();
  const cards = Array.from(document.querySelectorAll<HTMLElement>('.strand-card'));
  let best: SnapCandidate | null = null;
  for (const card of cards) {
    const id = card.dataset.strandId;
    if (!id || id === excludeId) continue;
    const cardRect = card.getBoundingClientRect();

    // Check horizontal proximity based on actual block position, not pointer
    if (ghostRect) {
      const horizMargin = 20;
      if (ghostRect.right < cardRect.left - horizMargin || ghostRect.left > cardRect.right + horizMargin) continue;
    } else {
      if (e.clientX < cardRect.left - 60 || e.clientX > cardRect.right + 60) continue;
    }

    const body = card.querySelector('.strand-body');
    const rows = Array.from(card.querySelectorAll('.instruction-row'));
    const boundaries = rows.map(r => r.getBoundingClientRect().top);
    boundaries.push(rows.length
      ? rows[rows.length - 1].getBoundingClientRect().bottom
      : (body?.getBoundingClientRect().top ?? cardRect.top) + 8);
    // Index 0 (the very top boundary) would attach something above the
    // strand's first block — never allowed when that first block is a
    // "When Ran", since nothing may ever end up underneath it.
    const strandInstructions = findStrand(id)?.instructions ?? [];
    const headIsWhenRan = strandInstructions[0] && isHeaderType(strandInstructions[0].type);
    for (let idx = 0; idx < boundaries.length; idx++) {
      if (idx === 0 && headIsWhenRan) continue;
      // A boundary directly below a cap block (Return) would attach
      // something underneath it — never allowed, since a cap block always
      // ends its strand's control flow.
      if (idx > 0 && isCapType(strandInstructions[idx - 1].type)) continue;
      const y = boundaries[idx];
      const refY = ghostRect ? ghostRect.top : e.clientY;
      const dist = Math.abs(refY - y);
      if (dist <= SNAP_THRESHOLD && (best === null || dist < (best as SnapCandidate).dist)) {
        // For the "after last row" boundary, the insertion point is 6px
        // above the boundary: the last row's margin-bottom: -6px means the
        // next item starts 6 px above its border-box bottom. That 6px is a
        // canvas-space (unscaled) constant, but `y` here is already a
        // measured, zoomed screen coordinate — scale the offset too, or it
        // over/undershoots the real gap at any zoom besides 1.
        const insY = idx === rows.length ? y - 6 * canvasZoom : y;
        best = { targetId: id, index: idx, dist, y: insY, left: cardRect.left, width: cardRect.width };
      }
    }
  }
  target.snap = best !== null ? { targetId: (best as SnapCandidate).targetId, index: (best as SnapCandidate).index } : null;

  if (best !== null) {
    showSnapPreview(best as SnapCandidate, ghostEl);
  } else {
    clearSnapPreview();
  }
}

let snapPreviewEl: HTMLElement | null = null;

function clearSnapPreview() {
  if (snapPreviewEl) {
    snapPreviewEl.remove();
    snapPreviewEl = null;
  }
}

function showSnapPreview(best: SnapCandidate, ghostEl?: HTMLElement) {
  if (snapPreviewEl) snapPreviewEl.remove();

  if (!ghostEl) return;
  const ghostRow = ghostEl.querySelector('.instruction-row');
  if (!ghostRow) return;

  const preview = document.createElement('div');
  preview.className = 'strand-snap-preview';

  const clone = ghostRow.cloneNode(true) as HTMLElement;
  clone.style.marginBottom = '0';
  preview.appendChild(clone);

  preview.style.left = `${best.left}px`;
  preview.style.top = `${best.y}px`;
  if (canvasZoom !== 1) {
    preview.style.transform = `scale(${canvasZoom})`;
    preview.style.transformOrigin = '0 0';
  }

  document.body.appendChild(preview);

  snapPreviewEl = preview;
}

export function clientToCanvas(clientX: number, clientY: number): [number, number] {
  const inner = document.getElementById('canvas-inner');
  if (!inner) return [0, 0];
  const rect = inner.getBoundingClientRect();
  return [
    Math.round((clientX - rect.left) / canvasZoom - CANVAS_PAD + lastBounds.minX),
    Math.round((clientY - rect.top) / canvasZoom - CANVAS_PAD + lastBounds.minY),
  ];
}

// The ghost is the real `.strand-card`/`.instruction-row` node(s), physically
// moved into it (not cloned, not hidden) — so the block being dragged is
// literally the strand itself following the pointer, not a stand-in.
function startDrag(e: PointerEvent, candidate: DragCandidate) {
  const { strandId, index, pointerId } = candidate;
  const strand = findStrand(strandId);
  if (!strand) return;

  const cardEl = document.querySelector<HTMLElement>(`.strand-card[data-strand-id="${cssEscape(strandId)}"]`);
  const wholeStrandGrab = index === 0;

  const ghost = document.createElement('div');
  ghost.className = 'strand-drag-ghost';
  let anchorRect: { left: number; top: number } = cardEl ? cardEl.getBoundingClientRect() : { left: e.clientX, top: e.clientY };
  let restoreCard: DragState['restoreCard'] = null;
  let hiddenRowEls: HTMLElement[] = [];

  if (wholeStrandGrab) {
    if (cardEl) {
      restoreCard = { el: cardEl, parent: cardEl.parentNode as Node, next: cardEl.nextSibling };
      // The card's left/top are canvas-space coordinates meant for its usual
      // absolutely-positioned home; inside the fixed-position ghost they'd
      // just add an unwanted offset on top of the pointer-tracked transform,
      // so drop them while it's on loan and let the ghost alone place it.
      cardEl.style.position = 'static';
      cardEl.style.left = '';
      cardEl.style.top = '';
      ghost.appendChild(cardEl);
    }
  } else {
    const rowEls = cardEl ? Array.from(cardEl.querySelectorAll<HTMLElement>('.instruction-row')).slice(index) : [];
    if (rowEls[0]) anchorRect = rowEls[0].getBoundingClientRect();
    const ghostCard = document.createElement('div');
    ghostCard.className = 'strand-card';
    const ghostBody = document.createElement('div');
    ghostBody.className = 'strand-body';
    rowEls.forEach(el => {
      // Clone first, hide second — cloneNode copies the inline style
      // attribute verbatim, so hiding the original before cloning made the
      // clone itself invisible too, leaving the ghost blank for the whole
      // drag.
      const clone = el.cloneNode(true) as HTMLElement;
      clone.style.visibility = '';
      ghostBody.appendChild(clone);
      el.style.visibility = 'hidden';
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
    restoreCard,
    hiddenRowEls,
    hiddenNewCardEl: null,
    blockId: wholeStrandGrab && strand.instructions[0]?.type === 'BlockHeader' ? strand.instructions[0].block_id : null,
    noSnap: wholeStrandGrab && strand.instructions[0] != null && isHeaderType(strand.instructions[0].type),
    preExistingStrandIds: new Set(state.current_macro?.strands?.map(s => s.id) ?? []),
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
    // A brand-new "When Ran" always becomes its own detached strand — it
    // can never snap into an existing one (it would either attach itself
    // underneath something, or land anywhere but the required index 0).
    if (isOverSidebar(e) || isHeaderType(paletteDrag.insType)) {
      paletteDrag.snap = null;
      clearSnapPreview();
    } else {
      updateSnapTarget(e, paletteDrag, null, paletteDrag.ghostEl);
    }
    return;
  }
  if (drag && e.pointerId === drag.pointerId) {
    positionGhost(e);
    if (isOverSidebar(e)) {
      drag.overTrash = true;
      drag.snap = null;
      clearSnapPreview();
      setSidebarArmed(true);
    } else {
      drag.overTrash = false;
      setSidebarArmed(false);
      if (drag.noSnap) {
        drag.snap = null;
        clearSnapPreview();
      } else {
        updateSnapTarget(e, drag, drag.resolvedId ?? dragCandidate?.strandId, drag.ghostEl);
      }
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
    // The primary-selection paste fires off the middle-button *release*,
    // dispatched by the native layer slightly after this pointerup handler
    // runs — clearing the flag synchronously here would re-open the window
    // right before the paste event arrives. Let it ride out a tick first.
    // Guard with a generation counter so this stale timer can't clear the
    // flag out from under a fresh pan that started in the meantime.
    const generation = blockPrimaryPasteGeneration;
    setTimeout(() => {
      if (blockPrimaryPasteGeneration === generation) blockPrimaryPaste = false;
    }, 200);
    document.getElementById('canvas-scroll')?.classList.remove('panning');
  }
  if (dragCandidate && dragCandidate.pointerId === e.pointerId) {
    dragCandidate = null;
  }

  if (paletteDrag && paletteDrag.pointerId === e.pointerId) {
    const finished = paletteDrag;
    paletteDrag = null;
    clearSnapPreview();
    finished.ghostEl.remove();

    if (!isOverSidebar(e)) {
      const ins = finished.insType === 'CallBlock' && finished.blockId
        ? paletteCallInstructionFor(finished.blockId)
        : clonePaletteInstruction(finished.insType);
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
  clearSnapPreview();
  setSidebarArmed(false);
  // Move the real card back to where it came from before removing the
  // (now-empty) ghost wrapper — unconditionally (move/merge/trash/error), so
  // Vue finds it exactly where its vnode tree still expects it on the next
  // patch. The backend round trip that would otherwise settle this is async
  // and, for trash/merge, may never re-render this node at all.
  if (finished.restoreCard) {
    finished.restoreCard.el.style.position = '';
    finished.restoreCard.parent.insertBefore(finished.restoreCard.el, finished.restoreCard.next);
  }
  // Remove the ghost first so the real card at the split point isn't
  // visible alongside the ghost's clones — that brief overlap reads as a
  // "copy of the dragged block left behind at the original position".
  finished.ghostEl.remove();
  // The split-drag rows were only ever hidden (their DOM parent never
  // changed), so just make them visible again — see the hiddenRowEls comment
  // on DragState for why they can't be physically moved like the card above.
  // For split-drags these references may be stale (Vue re-rendered the
  // original strand after splitStrand resolved), but the unhide is harmless.
  for (const el of finished.hiddenRowEls) el.style.visibility = '';
  // Don't unhide hiddenNewCardEl here — for merge/trash the strand is
  // removed by the backend so it should stay hidden; for the move case it's
  // unhidden in the async callback after the optimistic x/y update so the
  // card appears at the drop position, not the split point.

  void (async () => {
    const id = finished.resolvedId ?? (finished.resolvingPromise ? await finished.resolvingPromise : null);
    if (!id) return;
    if (finished.overTrash) {
      // A custom block's header strand: delete the block definition (which
      // also removes this body strand and every call site referencing it)
      // rather than just detaching the strand and leaving an orphaned
      // "My Blocks" prefab behind.
      if (finished.blockId) {
        await deleteBlock(finished.blockId);
      } else {
        await removeStrand(id);
      }
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
      // Reveal the card now that it's positioned at the drop coordinates,
      // so it never appears at the split point.
      if (finished.hiddenNewCardEl) finished.hiddenNewCardEl.style.visibility = '';
      await moveStrand(id, x, y);
    }
  })();
}

let listenersAttached = false;

/** Wires up the document/canvas-scroll-level pointer listeners. Idempotent. */
export function attachDragListeners() {
  if (listenersAttached) return;
  listenersAttached = true;
  document.addEventListener('pointerdown', beginPan);
  document.addEventListener('pointermove', onPointerMove);
  document.addEventListener('pointerup', onPointerUp);
  document.addEventListener('pointercancel', onPointerUp);
  document.addEventListener('paste', blockMiddleClickPaste, true);
  document.addEventListener('wheel', onCanvasWheel, { passive: false });
}
