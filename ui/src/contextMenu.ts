// Right-click context menu state — reactive singleton, same module-level
// pattern as dropdownRegistry.ts, but holds enough info (position + what was
// clicked) for a single <ContextMenu> component to render either the
// block-menu or canvas-menu item list.
import { reactive } from 'vue';
import { registerOpen, unregisterOpen, clientToCanvas } from 'blockstitch';
import type { InstrPath } from './types';

export type ContextMenuType = 'block' | 'canvas' | 'variable' | 'myBlock';

interface ContextMenuState {
  open: boolean;
  x: number;
  y: number;
  type: ContextMenuType;
  strandId: string;
  path: InstrPath;
  canvasX: number;
  canvasY: number;
  variableName: string;
  blockId: string;
}

export const contextMenu = reactive<ContextMenuState>({
  open: false,
  x: 0,
  y: 0,
  type: 'block',
  strandId: '',
  path: [],
  canvasX: 0,
  canvasY: 0,
  variableName: '',
  blockId: '',
});

function openAt(e: MouseEvent) {
  registerOpen(closeContextMenu);
  contextMenu.open = true;
  contextMenu.x = e.clientX;
  contextMenu.y = e.clientY;
}

export function openBlockMenu(e: MouseEvent, strandId: string, path: InstrPath): void {
  openAt(e);
  contextMenu.type = 'block';
  contextMenu.strandId = strandId;
  contextMenu.path = path;
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

export function openMyBlockMenu(e: MouseEvent, blockId: string): void {
  openAt(e);
  contextMenu.type = 'myBlock';
  contextMenu.blockId = blockId;
}

export function closeContextMenu(): void {
  if (!contextMenu.open) return;
  contextMenu.open = false;
  unregisterOpen(closeContextMenu);
}
