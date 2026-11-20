// The keys of the layout: panels and the shortcut hint. Pure: the editor decides what to do with the answer.

export type LayoutKey = 'layers' | 'inspector' | 'help' | 'palette';

interface KeyLike {
  key: string;
  code: string;
  altKey: boolean;
  shiftKey: boolean;
  ctrlKey: boolean;
  metaKey: boolean;
}

/**
 * Option+1 and Option+2 toggle the left and right panels (by physical key: Option changes the character on a Mac); `?`
 * opens the hint; Ctrl+K, or Command+K on a Mac, opens the palette (Shift and Option make another key).
 */
export function layoutKey(e: KeyLike): LayoutKey | null {
  if (e.ctrlKey || e.metaKey) return e.key.toLowerCase() === 'k' && !e.altKey && !e.shiftKey ? 'palette' : null;
  if (e.altKey) return e.code === 'Digit1' ? 'layers' : e.code === 'Digit2' ? 'inspector' : null;
  return e.key === '?' ? 'help' : null;
}

/** A menu or a dialog that is open on the page: its role and its test id. */
export interface OpenOverlay {
  role: string;
  id: string | null;
}

/** The open menus and dialogs under `root` (Bits UI gives them `role=menu` and `role=dialog`; a closed one is not in the page). */
export function readOverlays(root: { querySelectorAll(selector: string): Iterable<{ getAttribute(name: string): string | null }> }): OpenOverlay[] {
  return [...root.querySelectorAll('[role=menu], [role=dialog]')].map((e) => ({ role: e.getAttribute('role') ?? '', id: e.getAttribute('data-testid') }));
}

/**
 * Whether Ctrl+K must leave the palette shut: a menu or a dialog is open, or the Export dialog is (two overlays would
 * stack and the first stays up behind the palette). The shortcut sheet is not in the way (the palette replaces it), and
 * the palette itself does not block its own key.
 */
export function paletteBlocked(open: readonly OpenOverlay[], exportOpen: boolean): boolean {
  return exportOpen || open.some((o) => o.id !== 'palette' && o.id !== 'shortcut-hint');
}
