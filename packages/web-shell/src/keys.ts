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

/**
 * Whether a menu is open on the page (Bits UI and Base UI give an open menu `role=menu`; a closed one is not in the page). A menu keeps its
 * open state in the component library, so the app layer cannot know it: the page does not send Ctrl+K while one is open, as it does not send
 * a key that a field owns (`ownsKeys`). The dialogs and the bind picker are the app layer's state, and it keeps the palette shut over them
 * (`OverlayOp`).
 */
export function menuOpen(root: { querySelector(selector: string): unknown }): boolean {
  return root.querySelector('[role=menu]') !== null;
}
