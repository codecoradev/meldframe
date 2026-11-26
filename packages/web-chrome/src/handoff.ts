// Where the focus goes when an overlay closes, for the React copies on Base UI (docs/react-web-plan.md, section 5, "Focus
// handoff"). One `Handoff` belongs to one overlay (a menu, the Export dialog, the palette). It sits on the one-element slot
// of `focus.ts` (`rememberFocus`, `takeFocusBack`) that the Svelte copies use directly with Bits UI's close event.
//
// The rule that shapes it: Base UI resolves a popup's `finalFocus` in the cleanup of its focus manager's effect, which runs
// on every change of that effect's dependencies and not only on a close, and may so run more than once. `finalFocus` here is
// therefore a plain read of what was decided earlier: when the overlay opened (`opened` takes what an earlier overlay left
// in the slot) or when a row or an entry chose to open a dialog (`pass`). Nothing is spent, moved or run inside it; the
// work that must happen once runs on the close event (`closed`).
import { type Focusable, rememberFocus, takeFocusBack } from './focus';

/** `E` is the kind of element the overlays of one framework pass around; React's callers say `HTMLElement`, which is what Base UI takes. */
export interface Handoff<E extends Focusable = Focusable> {
  /** The element that had the focus when the overlay opened; null before the first open. */
  readonly opener: E | null;
  /**
   * The overlay opened, with `opener` holding the focus. Takes what an earlier overlay left for it and drops the decision
   * and the pending entry of the last close, so every open starts clean. Call it once per open, before the overlay moves
   * the focus; `useFocusHandoff` does.
   */
  opened(opener: E | null): void;
  /**
   * The row or entry chosen opens a dialog, which takes the focus: this overlay leaves it alone when it closes, `to` is
   * left in the slot for that dialog to give the focus back to (null clears the slot), and `then`, when given, runs once
   * when the close is complete (`closed`), so the dialog opens after this overlay let go.
   */
  pass(to: E | null, then?: () => void): void;
  /**
   * For the popup's `finalFocus`: the element to give the focus to, `false` to leave it where it is, or null for the
   * library's own return (the element that had it, or the trigger). Pure: the same answer however often it is asked.
   */
  finalFocus(): E | false | null;
  /** The close is complete (Base UI's `onOpenChangeComplete(false)`): runs what `pass` was given, once. A throw reaches the caller after the handoff is clean. */
  closed(): void;
}

export function createHandoff<E extends Focusable = Focusable>(): Handoff<E> {
  let opener: E | null = null;
  let inherited: E | null = null;
  let passed = false;
  let then: (() => void) | null = null;
  return {
    get opener() {
      return opener;
    },
    opened(el) {
      opener = el;
      inherited = takeFocusBack() as E | null; // the slot holds what the overlays of this framework put there
      passed = false;
      then = null;
    },
    pass(to, after) {
      passed = true;
      then = after ?? null;
      rememberFocus(to);
    },
    finalFocus() {
      if (passed) return false;
      return inherited?.isConnected ? inherited : null;
    },
    closed() {
      const f = then;
      then = null;
      f?.();
    },
  };
}
