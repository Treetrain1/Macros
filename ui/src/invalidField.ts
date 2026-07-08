import { state } from './store';

/**
 * Looks up the backend's echoed raw text for a numeric field currently being
 * edited (present while the typed text doesn't parse to a valid value yet)
 * and whether it's still invalid.
 */
export function getInvalidText(strandId: string, index: number, fieldId: string): { text: string; invalid: boolean } | null {
  const entry = state.invalid_field_buffers.find(
    b => b.strand_id === strandId && b.instruction_index === index && b.field_id === fieldId,
  );
  if (!entry) return null;
  const trimmed = entry.text.trim();
  let invalid = true;
  if (trimmed !== '') {
    const num = Number(trimmed);
    if (!isNaN(num)) {
      invalid = fieldId === 'WaitDuration' || fieldId === 'WaitRandomness' ? false : !Number.isInteger(num);
    }
  }
  return { text: entry.text, invalid };
}
