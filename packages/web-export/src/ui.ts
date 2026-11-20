// The view state of the Export dialog and what its buttons do, as pure steps. The dialog is built by the app layer
// (`state.export.dialog`); this is what the page keeps on top of it: the tab, the file shown and what the last Copy said.
import type { Action } from '@meldframe/protocol';
import type { Frame } from './png';

export interface Said {
  ok: boolean;
  text: string;
}
export interface View {
  tab: 'html' | 'png';
  shown: 'html' | 'css';
  said: Said | null;
}
export const START: View = { tab: 'html', shown: 'html', said: null };

/**
 * The view after the dialog changed: a dialog that opened, closed or moved to another frame starts again (so the next
 * one does not show the last one's tab or its "copied"); the same frame keeps what the person did.
 */
export function viewFor(prev: { frame: string | null; view: View }, frame: string | null): { frame: string | null; view: View } {
  return frame === prev.frame ? prev : { frame, view: START };
}

/** A change of the format shows the HTML file again, and what the last Copy said no longer holds. */
export const afterFormat = (v: View): View => ({ ...v, shown: 'html', said: null });

/** The frame the dialog acts on, for its PNG: its own, not whatever is selected now. */
export function dialogFrame(d: { frame: string; name: string } | null): Frame | null {
  return d ? { id: d.frame, name: d.name } : null;
}

/** Review: the dialog closes and the right panel opens (the page then selects its Activity tab). */
export const reviewSteps = (): Action[] => [{ type: 'closeExport' }, { type: 'setPanel', panel: 'properties', open: true }];
