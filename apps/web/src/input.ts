/** A number typed into a field: finite, or null (an empty or odd field must never become 0). */
export function parseNumber(text: string): number | null {
  if (text.trim() === '') return null;
  const n = Number(text);
  return Number.isFinite(n) ? n : null;
}
