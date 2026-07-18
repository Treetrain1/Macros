import { state } from './store';
import type { ValueLocationDto } from './types';

function locationsEqual(a: ValueLocationDto, b: ValueLocationDto): boolean {
  if (a.kind !== b.kind) return false;
  if (a.path.length !== b.path.length || !a.path.every((p, i) => p === b.path[i])) return false;
  if (a.kind === 'Field' && b.kind === 'Field') {
    return a.strand_id === b.strand_id && a.index === b.index && a.field_id === b.field_id;
  }
  if (a.kind === 'Floating' && b.kind === 'Floating') return a.floating_id === b.floating_id;
  return false;
}

/**
 * Looks up the backend's echoed raw text for a numeric leaf currently being
 * edited (present while the typed text doesn't parse to a valid value yet)
 * and whether it's still invalid. Fields tied to a pixel-valued instruction
 * slot (MoveMouseX/Y, ScrollAmount) require an integer; everything else
 * (Wait's duration/randomness, and every floating value block, which isn't
 * tied to any specific field) allows decimals — mirrors the backend's
 * `location_requires_integer`.
 */
export function getInvalidText(location: ValueLocationDto): { text: string; invalid: boolean } | null {
  const entry = state.invalid_field_buffers.find(b => locationsEqual(b.location, location));
  if (!entry) return null;
  const trimmed = entry.text.trim();
  let invalid = true;
  if (trimmed !== '') {
    const num = Number(trimmed);
    if (!isNaN(num)) {
      const fieldId = location.kind === 'Field' ? location.field_id : null;
      invalid = fieldId === 'MoveMouseX' || fieldId === 'MoveMouseY' || fieldId === 'ScrollAmount' ? !Number.isInteger(num) : false;
    }
  }
  return { text: entry.text, invalid };
}
