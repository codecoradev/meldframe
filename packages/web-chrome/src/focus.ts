// Shared class lists and the keyboard rule of the chrome primitives. Whole class names are written out so that
// Tailwind finds them; sizes come from the size tokens, never from a literal.

/** The focus ring of DESIGN.md: 2 px in the selection colour with a 2 px offset, on keyboard focus (or a preview). */
export const ring =
  'foc:outline-(length:--mf-stroke-focus) foc:outline-offset-(--mf-stroke-focus-offset) foc:outline-selection';
/** The same ring when an input inside the control has focus. */
export const ringWithin =
  'foc-in:outline-(length:--mf-stroke-focus) foc-in:outline-offset-(--mf-stroke-focus-offset) foc-in:outline-selection';
/** A disabled control: half opacity, no pointer, no hover (DESIGN.md 5.1). */
export const disabled = 'disabled:pointer-events-none disabled:opacity-(--mf-opacity-disabled)';

/** A disabled control that is not a button (a group, a field): the same dimming. */
export const dim = 'pointer-events-none opacity-(--mf-opacity-disabled)';

/** Square control sizes in px, as the size tokens `control-sm` to `control-xl`. */
export type ControlSize = 24 | 28 | 32 | 36;
export const square: Record<ControlSize, string> = {
  24: 'size-(--mf-size-control-sm)',
  28: 'size-(--mf-size-control-md)',
  32: 'size-(--mf-size-control-lg)',
  36: 'size-(--mf-size-control-xl)',
};

/** Field heights: dense 28, default 32, wide 34 (DESIGN.md 5.1). */
export type FieldSize = 'dense' | 'default' | 'wide';
export const fieldHeight: Record<FieldSize, string> = {
  dense: 'h-(--mf-size-control-md)',
  default: 'h-(--mf-size-control-lg)',
  wide: 'h-(--mf-size-control-wide)',
};

/** An accessible name is not optional: an empty one is a programming error, thrown where it is made. */
export function requireLabel(label: string | undefined, what: string): string {
  if (!label || !label.trim()) throw new Error(`${what} needs a label: an icon-only control has no other name`);
  return label;
}

/**
 * The index a key moves to in a roving group (radio group, tab list), or undefined when the key does nothing.
 * `off[i]` marks a disabled member: the arrows and Home and End skip it, and a group with none enabled does nothing.
 */
export function nextIndex(key: string, index: number, count: number, off: readonly boolean[] = []): number | undefined {
  if (count <= 0) return undefined;
  const step = key === 'ArrowRight' || key === 'ArrowDown' ? 1 : key === 'ArrowLeft' || key === 'ArrowUp' ? -1 : 0;
  const from = key === 'Home' ? -1 : key === 'End' ? count : index;
  const dir = key === 'Home' ? 1 : key === 'End' ? -1 : step;
  if (dir === 0) return undefined;
  for (let n = 1; n <= count; n++) {
    const at = (((from + dir * n) % count) + count) % count;
    if (!off[at]) return at;
  }
  return undefined;
}

/** What can be given the focus back: an element that may have left the page. */
export interface Focusable {
  isConnected: boolean;
  focus(): void;
}
let back: Focusable | null = null;

/** A menu row that opens a dialog remembers the menu's button here, so the dialog can give the focus back to it when it closes. */
export function rememberFocus(el: Focusable | null): void {
  back = el;
}

/** The remembered element, once; null when none was remembered or it left the page. */
export function takeFocusBack(): Focusable | null {
  const el = back;
  back = null;
  return el?.isConnected ? el : null;
}

