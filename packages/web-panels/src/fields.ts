// What the sections of the Properties panel read in common, apart from the markup. Both copies of the panel (Svelte and React)
// call these, so the number format, the sending of an action and the focus rule of the image picker are not written twice.
// Presentation only: what a typed value becomes is the app layer's (`Action::Typed`, see web-document/properties.ts).
import type { Action, Properties } from '@meldframe/protocol';

/** What every section takes: the node's properties as the app layer gave them, and where to send an action. */
export interface SectionProps {
  properties: Properties;
  dispatch: (a: Action) => void;
}

/** A number as a field shows it: two decimals at most, no trailing zero. */
export const round2 = (v: number): string => String(Math.round(v * 100) / 100);

/** Sends what a field made, and nothing when it made nothing (a value that did not change is no action). */
export const sender =
  (dispatch: (a: Action) => void) =>
  (a: Action | null): void => {
    if (a) dispatch(a);
  };

/**
 * The image picker closed for this node (a choice, Escape or its button): `was` is the node it was open for last time (null when it
 * was closed), `open` whether it is open now. A picker that closed because another node was selected is not this case, so focus
 * is left alone then.
 */
export const pickerClosed = (was: string | null, id: string, open: boolean): boolean => was === id && !open;
