// Right-click context menu state — reactive singleton, same module-level
// pattern as dropdownRegistry.ts, but holds enough info (position + what was
// clicked) for a single <ContextMenu> component to render either the
// block-menu or canvas-menu item list.
import { reactive } from 'vue';
import { registerOpen, unregisterOpen } from './dropdownRegistry';
import { clientToCanvas } from './canvasDrag';

export type ContextMenuType = 'block' | 'canvas' | 'variable';

interface ContextMenuState {
  open: boolean;
  x: number;
  y: number;
  type: ContextMenuType;
  strandId: string;
  index: number;
  canvasX: number;
  canvasY: number;
  variableName: string;
}

export const contextMenu = reactive<ContextMenuState>({
  open: false,
  x: 0,
  y: 0,
  type: 'block',
  strandId: '',
  index: 0,
  canvasX: 0,
  canvasY: 0,
  variableName: '',
});

function openAt(e: MouseEvent) {
  registerOpen(closeContextMenu);
  contextMenu.open = true;
  contextMenu.x = e.clientX;
  contextMenu.y = e.clientY;
}

export function openBlockMenu(e: MouseEvent, strandId: string, index: number): void {
  openAt(e);
  contextMenu.type = 'block';
  contextMenu.strandId = strandId;
  contextMenu.index = index;
}

export function openCanvasMenu(e: MouseEvent): void {
  openAt(e);
  contextMenu.type = 'canvas';
  const [cx, cy] = clientToCanvas(e.clientX, e.clientY);
  contextMenu.canvasX = cx;
  contextMenu.canvasY = cy;
}

export function openVariableMenu(e: MouseEvent, name: string): void {
  openAt(e);
  contextMenu.type = 'variable';
  contextMenu.variableName = name;
}

export function closeContextMenu(): void {
  if (!contextMenu.open) return;
  contextMenu.open = false;
  unregisterOpen(closeContextMenu);
}
