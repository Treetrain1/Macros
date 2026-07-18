import { ref } from 'vue';

// Module-level singleton — one sidebar width for the whole app's lifetime,
// same pattern as useTheme.ts's currentTheme.
const STORAGE_KEY = 'macros-sidebar-width';
const DEFAULT_WIDTH = 348;
const MIN_WIDTH = 180;
const MAX_WIDTH = 600;

function clamp(width: number): number {
  return Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, width));
}

function loadWidth(): number {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    const n = raw === null ? NaN : Number(raw);
    return isNaN(n) ? DEFAULT_WIDTH : clamp(n);
  } catch (_e) {
    return DEFAULT_WIDTH;
  }
}

export const sidebarWidth = ref(loadWidth());

function persist(width: number) {
  try {
    localStorage.setItem(STORAGE_KEY, String(width));
  } catch (_e) {
    // ignore (e.g. storage disabled)
  }
}

let dragPointerId: number | null = null;
let startX = 0;
let startWidth = 0;

function onPointerMove(e: PointerEvent) {
  if (e.pointerId !== dragPointerId) return;
  sidebarWidth.value = clamp(startWidth + (e.clientX - startX));
}

function endDrag(e: PointerEvent) {
  if (e.pointerId !== dragPointerId) return;
  dragPointerId = null;
  document.removeEventListener('pointermove', onPointerMove);
  document.removeEventListener('pointerup', endDrag);
  document.removeEventListener('pointercancel', endDrag);
  document.body.classList.remove('sidebar-resizing');
  persist(sidebarWidth.value);
}

/** Pointer-drag resize of the instruction sidebar, via its edge handle —
 * same "attach listeners for the duration of the drag" shape as
 * canvasDrag.ts's pan handling, kept self-contained here since nothing else
 * needs to know about sidebar-resize state. */
export function beginSidebarResize(e: PointerEvent) {
  if (e.button !== undefined && e.button !== 0) return;
  e.preventDefault();
  dragPointerId = e.pointerId;
  startX = e.clientX;
  startWidth = sidebarWidth.value;
  document.body.classList.add('sidebar-resizing');
  document.addEventListener('pointermove', onPointerMove);
  document.addEventListener('pointerup', endDrag);
  document.addEventListener('pointercancel', endDrag);
}
