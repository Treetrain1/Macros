import type { HotkeyActionDto } from './types';

export type NamedActionType = Exclude<HotkeyActionDto['type'], 'RunSpecificMacro'>;

export const NAMED_ACTIONS: { label: string; type: NamedActionType }[] = [
  { label: 'Run Macro', type: 'RunMacro' },
  { label: 'Stop Loop', type: 'StopLoop' },
  { label: 'Next Macro', type: 'NextMacro' },
  { label: 'Previous Macro', type: 'PrevMacro' },
  { label: 'Toggle Loop', type: 'ToggleLoop' },
  { label: 'Start Recording (immediate)', type: 'StartRecordingImmediate' },
];
