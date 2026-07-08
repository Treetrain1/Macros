// Only one AppDropdown panel is ever open at a time — module-level singleton
// tracking whichever one is currently open so opening a second one closes it.
let activeClose: (() => void) | null = null;

export function registerOpen(close: () => void): void {
  if (activeClose && activeClose !== close) activeClose();
  activeClose = close;
}

export function unregisterOpen(close: () => void): void {
  if (activeClose === close) activeClose = null;
}

export function closeAllDropdowns(): void {
  if (activeClose) activeClose();
}
