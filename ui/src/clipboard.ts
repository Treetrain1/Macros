// Right-click copy/paste clipboard — in-memory only (not the OS clipboard),
// module-level singleton same as dropdownRegistry.ts. Lives for the app
// session; there's no need for it to survive a reload.
import type { InstructionDto, StrandDto } from './types';

let copied: InstructionDto[] | null = null;

export function copyBlock(strand: StrandDto, index: number): void {
  const ins = strand.instructions[index];
  if (ins) copied = [ins];
}

export function copyAll(strand: StrandDto, index: number): void {
  copied = strand.instructions.slice(index);
}

export function hasClipboard(): boolean {
  return copied !== null && copied.length > 0;
}

export function clipboardContents(): InstructionDto[] | null {
  return copied;
}
