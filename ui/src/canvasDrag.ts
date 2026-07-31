// Imperative Scratch-style strand-canvas drag/drop/pan/zoom machinery, ported
// near-verbatim from the original vanilla app.js. Deliberately not a
// reactive/declarative rewrite — it's a real-time pointer/DOM-measurement
// state machine Vue's reactivity graph has no visibility into.
import { state } from './store';
import { addInstruction, addStrand, deleteBlock, mergeStrand, moveStrand, removeStrand, splitStrand } from './tauri';
import { clonePaletteInstruction } from './paletteState';
import { paletteCallInstructionFor } from './blockDefs';
import type { InstrPath, InstructionDto, MacroDto, PathStep } from './types';
import { isCapType, isHeaderType, resolveInstructionList } from './types';

// Structural equality for two path *prefixes* (a basePath, not necessarily a
// full instruction address) — `slot` is normalized so an omitted/undefined
// slot compares equal to itself regardless of which side omitted it.
function pathPrefixEqual(a: PathStep[], b: PathStep[]): boolean {
  return a.length === b.length && a.every((s, i) => s.index === b[i].index && (s.slot ?? null) === (b[i].slot ?? null));
}

function parseBasePath(el: HTMLElement): PathStep[] | null {
  try {
    return JSON.parse(el.dataset.path ?? '[]');
  } catch {
    return null;
  }
}

/** Finds the `.instruction-list` DOM container addressed by `basePath` — a
 * strand's own top-level list (`basePath: []`) or a nested If/IfElse body. */
function findListContainer(strandId: string, basePath: PathStep[]): HTMLElement | null {
  const containers = document.querySelectorAll<HTMLElement>('.instruction-list');
  for (const el of containers) {
    if (el.dataset.strandId !== strandId) continue;
    const parsed = parseBasePath(el);
    if (parsed && pathPrefixEqual(parsed, basePath)) return el;
  }
  return null;
}


// Strand x/y can go negative; canvas-inner is sized to the strands' bounding
// box each render, padded so cards never touch the edge.
const CANVAS_PAD = 400;
// Bounds always extend at least this far past the origin, so there's room
// to pan even when a macro has only one strand at (0, 0).
const ROOT_MARGIN = 1000;

function cssEscape(s: string): string {
  return String(s).replace(/[^a-zA-Z0-9_-]/g, '\\$&');
}

function findStrand(strandId: string) {
  return state.current_macro?.strands?.find(st => st.id === strandId);
}

// ── Zoom (ctrl+scroll) ──────────────────────────────────────────────────────
// canvas-inner stays laid out at unscaled coordinates and is visually scaled
// via CSS transform; canvas-sizer's box matches the zoomed footprint so
// scrollbars match what's on screen.
const MIN_ZOOM = 0.35;
const MAX_ZOOM = 2.5;
let canvasZoom = 1;
let currentMacroId: string | null = null;

// Value-block drag ghosts (valueDrag.ts) live outside canvas-inner's scaled
// subtree too, so they need this to counter-scale themselves.
export function getCanvasZoom(): number {
  return canvasZoom;
}

// Bounds of the last render, needed to convert pointer/client coordinates
// back into canvas (strand.x/y) space when a drag ends on empty canvas.
let lastBounds = { minX: 0, minY: 0 };

/** DOM geometry pass: measures rendered strand cards and positions them in
 * canvas space. Call after Vue patches the DOM (e.g. a `flush: 'post'` watcher). */
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
  // A strand beyond the previous extent pushes minX/minY outward, shifting
  // every card's position — compensate scroll so nothing visibly jumps.
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

  // A split-drag's new strand can render as a real, visible card at the split
  // point before drop — hide it for the drag's duration (same idea as the
  // whole-strand-grab ghost). resolvedId is set async by splitStrand, but
  // state-updated can arrive first, so preExistingStrandIds catches it too.
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
      // spawn near (0, 0).
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

// On Linux, middle click is X11's "paste primary selection" — CEF honors it
// against the focused element regardless of hit-testing, so preventDefault()
// on pointerdown doesn't stop it. Swallow the resulting ClipboardEvent
// instead while a middle-click pan is in flight.
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
  path: InstrPath;
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
  snap: { targetId: string; path: InstrPath } | null;
  overTrash?: boolean;
  // Set for a whole-strand grab of a BlockHeader strand — its custom block's
  // id, so trashing it deletes the block definition (deleteBlock) instead of
  // just detaching the body strand and orphaning the "My Blocks" prefab.
  blockId: string | null;
  // A "When Ran"-headed strand can be moved freely but never snapped/merged
  // into another (that would attach it underneath something).
  noSnap: boolean;
  // The real DOM node(s) are physically relocated into the ghost (not
  // cloned), so the strand visibly follows the pointer. Vue reuses these
  // nodes by key, so they MUST be moved back explicitly on every drag end
  // or Vue's next patch gets confused about where they live.
  restoreCard: { el: HTMLElement; parent: Node; next: Node | null } | null;
  // Partial (split) drags can't use the real-node move: splitStrand's
  // response can patch the old strand's row list mid-drag while rows sit
  // detached in the ghost, so these stay hidden clones instead.
  hiddenRowEls: HTMLElement[];
  // A resolved split's new strand renders visible at the split point almost
  // immediately; positionCanvas hides it each render — tracked here so
  // pointerup knows what to unhide.
  hiddenNewCardEl: HTMLElement | null;
  // Strand IDs that existed when the drag started — lets positionCanvas
  // detect new split-strand cards before resolvedId is set.
  preExistingStrandIds: Set<string>;
}
let drag: DragState | null = null;

interface PaletteDragState {
  pointerId: number;
  insType: InstructionDto['type'];
  // Set only for a `CallBlock` drag from "My Blocks" — clonePaletteInstruction
  // can't resolve which custom block from `insType` alone.
  blockId?: string;
  offsetX: number;
  offsetY: number;
  ghostEl: HTMLElement;
  snap: { targetId: string; path: InstrPath } | null;
}
let paletteDrag: PaletteDragState | null = null;

export function beginPickup(e: PointerEvent, strandId: string, path: InstrPath) {
  if (state.recording_phase.phase === 'Active') return;
  if (e.button !== undefined && e.button !== 0) return;
  e.preventDefault();
  capturePointer(e);
  dragCandidate = { strandId, path, pointerId: e.pointerId, startX: e.clientX, startY: e.clientY };
}

// Dragging a palette entry previews the actual instruction block it creates
// (not the sidebar chip).
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

const SNAP_THRESHOLD = 36;

// Shared by strand-drags and palette-drags; writes the result onto
// `target.snap` and updates the shared snap preview.
interface SnapCandidate { targetId: string; path: InstrPath; dist: number; y: number; left: number; width: number }

// Scans every `.instruction-list` container on the canvas (a strand's own
// top level, or a nested If/IfElse body) independently — each contributes
// its own boundary set from only its *direct* child rows, so a nested
// body's rows never leak into an ancestor's boundaries and vice versa — then
// picks the single globally closest boundary across all of them.
function updateSnapTarget(e: PointerEvent, target: { snap: { targetId: string; path: InstrPath } | null }, excludeId: string | null | undefined, ghostEl?: HTMLElement) {
  const ghostRect = ghostEl?.getBoundingClientRect();
  const containers = Array.from(document.querySelectorAll<HTMLElement>('.instruction-list'));
  let best: SnapCandidate | null = null;
  for (const container of containers) {
    const id = container.dataset.strandId;
    if (!id || id === excludeId) continue;
    const cardEl = container.closest<HTMLElement>('.strand-card');
    if (!cardEl) continue;
    const cardRect = cardEl.getBoundingClientRect();

    // Judge horizontal proximity against the strand's actual attach point
    // (left edge), not a loose bounding-box overlap.
    const refX = ghostRect ? ghostRect.left : e.clientX;
    if (Math.abs(refX - cardRect.left) > SNAP_THRESHOLD) continue;

    const basePath = parseBasePath(container);
    if (!basePath) continue;

    const rows = Array.from(container.children).filter((el): el is HTMLElement => el.classList.contains('instruction-row'));
    const containerRect = container.getBoundingClientRect();
    const boundaries = rows.map(r => r.getBoundingClientRect().top);
    boundaries.push(rows.length ? rows[rows.length - 1].getBoundingClientRect().bottom : containerRect.top + 8);

    // Index 0 would attach above this list's first block — never allowed
    // when that's a "When Ran"/BlockHeader (nothing may end up above it);
    // only possible at a strand's own top level (basePath: []), since a
    // header can never live nested inside an If/IfElse body.
    const listInstructions = resolveInstructionList(findStrand(id), basePath);
    const headIsWhenRan = basePath.length === 0 && listInstructions[0] && isHeaderType(listInstructions[0].type);
    for (let idx = 0; idx < boundaries.length; idx++) {
      if (idx === 0 && headIsWhenRan) continue;
      // A boundary below a cap block (Return) would attach something
      // underneath it — never allowed, since it ends this list's flow.
      if (idx > 0 && isCapType(listInstructions[idx - 1].type)) continue;
      const y = boundaries[idx];
      const refY = ghostRect ? ghostRect.top : e.clientY;
      const dist = Math.abs(refY - y);
      if (dist <= SNAP_THRESHOLD && (best === null || dist < best.dist)) {
        // The last row's margin-bottom: -6px means the next item starts 6px
        // above its border-box bottom. That's a canvas-space constant, but
        // `y` is a zoomed screen coordinate — scale the offset too.
        const insY = idx === rows.length ? y - 6 * canvasZoom : y;
        best = { targetId: id, path: [...basePath, { index: idx }], dist, y: insY, left: cardRect.left, width: cardRect.width };
      }
    }
  }
  target.snap = best !== null ? { targetId: best.targetId, path: best.path } : null;

  if (best !== null) {
    showSnapPreview(best, ghostEl);
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
// moved into it — the block dragged is literally the strand itself.
function startDrag(e: PointerEvent, candidate: DragCandidate) {
  const { strandId, path, pointerId } = candidate;
  const strand = findStrand(strandId);
  if (!strand) return;

  const cardEl = document.querySelector<HTMLElement>(`.strand-card[data-strand-id="${cssEscape(strandId)}"]`);
  // Only a true top-level index-0 grab picks up the whole strand card as a
  // unit — anything nested inside an If/IfElse body, or a non-zero
  // top-level index, always goes through the split-off-a-tail path below
  // (splitStrand generalizes to a nested body the same way it always has
  // for a top-level one, see commands.rs's resolve_body_mut).
  const wholeStrandGrab = path.length === 1 && path[0].index === 0;

  const ghost = document.createElement('div');
  ghost.className = 'strand-drag-ghost';
  let anchorRect: { left: number; top: number } = cardEl ? cardEl.getBoundingClientRect() : { left: e.clientX, top: e.clientY };
  let restoreCard: DragState['restoreCard'] = null;
  let hiddenRowEls: HTMLElement[] = [];

  if (wholeStrandGrab) {
    if (cardEl) {
      restoreCard = { el: cardEl, parent: cardEl.parentNode as Node, next: cardEl.nextSibling };
      // The card's left/top are canvas-space coordinates for its usual home;
      // drop them while on loan so the ghost's transform alone places it.
      cardEl.style.position = 'static';
      cardEl.style.left = '';
      cardEl.style.top = '';
      ghost.appendChild(cardEl);
    }
  } else {
    const basePath = path.slice(0, -1);
    const localIndex = path[path.length - 1].index;
    const container = findListContainer(strandId, basePath);
    const rowEls = container
      ? Array.from(container.children).filter((el): el is HTMLElement => el.classList.contains('instruction-row')).slice(localIndex)
      : [];
    if (rowEls[0]) anchorRect = rowEls[0].getBoundingClientRect();
    const ghostCard = document.createElement('div');
    ghostCard.className = 'strand-card';
    const ghostBody = document.createElement('div');
    ghostBody.className = 'strand-body';
    rowEls.forEach(el => {
      // Clone first, hide second — cloneNode copies inline style verbatim,
      // so hiding first would make the clone invisible too.
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
    newDrag.resolvingPromise = splitStrand(strandId, path, strand.x + 24, strand.y + 24)
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

// Each branch below confirms the event's pointerId belongs to that gesture
// before handling it, rather than a blanket "some gesture is active" check —
// a stuck gesture would otherwise swallow every other pointer's events forever.
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
    // can never snap into an existing one.
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
    // The primary-selection paste fires on middle-button release slightly
    // after this handler runs, so clear the flag after a tick, not
    // synchronously. Guard with a generation counter against a fresh pan.
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
            await addInstruction(finished.snap.targetId, finished.snap.path, ins);
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
  // Move the real card back before removing the ghost wrapper —
  // unconditionally, so Vue finds it where its vnode tree expects on the
  // next patch (the async backend round trip may never re-render it at all).
  if (finished.restoreCard) {
    finished.restoreCard.el.style.position = '';
    finished.restoreCard.parent.insertBefore(finished.restoreCard.el, finished.restoreCard.next);
  }
  // Remove the ghost first so the real card at the split point isn't
  // briefly visible alongside the ghost's clones.
  finished.ghostEl.remove();
  // The split-drag rows were only hidden (parent never changed), so unhide
  // them here (see hiddenRowEls on DragState for why they can't be moved
  // like the card). References may be stale post-resolve, but unhiding is harmless.
  for (const el of finished.hiddenRowEls) el.style.visibility = '';
  // Don't unhide hiddenNewCardEl here — merge/trash removes the strand so it
  // stays hidden; move unhides it after the optimistic x/y update below.

  void (async () => {
    const id = finished.resolvedId ?? (finished.resolvingPromise ? await finished.resolvingPromise : null);
    if (!id) return;
    if (finished.overTrash) {
      // A custom block's header strand: delete the block definition (which
      // removes the body strand and every call site) rather than just detaching it.
      if (finished.blockId) {
        await deleteBlock(finished.blockId);
      } else {
        await removeStrand(id);
      }
    } else if (finished.snap) {
      await mergeStrand(id, finished.snap.targetId, finished.snap.path);
    } else {
      const [x, y] = clientToCanvas(e.clientX - finished.offsetX, e.clientY - finished.offsetY);
      // Optimistic: apply locally so the card renders at the drop point
      // immediately instead of "teleporting" once the round trip lands.
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
