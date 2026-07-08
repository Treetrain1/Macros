// Lucide icons (https://lucide.dev) via lucide-vue-next — Vue component
// versions replacing the old lucide-static raw-SVG imports.
import type { Component } from 'vue';
import {
  ChevronUp,
  ChevronDown,
  Trash2,
  Plus,
  X,
  Settings,
  Play,
  Repeat,
  Circle,
  Square,
  Pause,
  CornerUpLeft,
  CornerUpRight,
  Save,
  ArrowLeft,
  Keyboard,
  TriangleAlert,
  Sun,
  Moon,
  RefreshCw,
  Server,
  Inbox,
  Clock,
  TextCursor,
  MousePointerClick,
  Move,
  Mouse,
  Terminal,
  MessageSquare,
  CornerDownRight,
} from 'lucide-vue-next';
import type { InstructionType } from './types';

export const ICONS = {
  'chevron-up': ChevronUp,
  'chevron-down': ChevronDown,
  trash: Trash2,
  plus: Plus,
  x: X,
  settings: Settings,
  play: Play,
  repeat: Repeat,
  circle: Circle,
  square: Square,
  pause: Pause,
  'corner-up-left': CornerUpLeft,
  'corner-up-right': CornerUpRight,
  save: Save,
  'arrow-left': ArrowLeft,
  key: Keyboard,
  'alert-triangle': TriangleAlert,
  sun: Sun,
  moon: Moon,
  'refresh-cw': RefreshCw,
  server: Server,
  inbox: Inbox,
  clock: Clock,
  'text-cursor': TextCursor,
  'mouse-pointer-click': MousePointerClick,
  move: Move,
  mouse: Mouse,
  terminal: Terminal,
  'message-square': MessageSquare,
  'corner-down-right': CornerDownRight,
} satisfies Record<string, Component>;

export type IconName = keyof typeof ICONS;

/** Icon for each instruction type, used by the sidebar palette. */
export const INSTRUCTION_TYPE_ICONS: Record<InstructionType, IconName> = {
  Wait: 'clock',
  Text: 'text-cursor',
  Key: 'key',
  Button: 'mouse-pointer-click',
  MoveMouse: 'move',
  Scroll: 'mouse',
  Command: 'terminal',
  Comment: 'message-square',
};

export const INSTRUCTION_TYPE_LABELS: Record<InstructionType, string> = {
  Wait: 'Wait',
  Text: 'Text',
  Key: 'Key',
  Button: 'Mouse Button',
  MoveMouse: 'Move Mouse',
  Scroll: 'Scroll',
  Command: 'Command',
  Comment: 'Comment',
};
