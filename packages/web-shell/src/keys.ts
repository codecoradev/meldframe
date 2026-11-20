// The keys of the layout: panels and the shortcut hint. Pure: the editor decides what to do with the answer.

export type LayoutKey = 'layers' | 'inspector' | 'help';

interface KeyLike {
  key: string;
  code: string;
  altKey: boolean;
  ctrlKey: boolean;
  metaKey: boolean;
}

/** Option+1 and Option+2 toggle the left and right panels (by physical key: Option changes the character on a Mac); `?` opens the hint. */
export function layoutKey(e: KeyLike): LayoutKey | null {
  if (e.ctrlKey || e.metaKey) return null;
  if (e.altKey) return e.code === 'Digit1' ? 'layers' : e.code === 'Digit2' ? 'inspector' : null;
  return e.key === '?' ? 'help' : null;
}
