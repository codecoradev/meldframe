// The shortcuts the editor answers to today, as the sheet lists them and as the menus show them. Pure. The table is
// checked against the real handlers in shortcuts.test.ts: a row the editor does not answer to fails there, so the sheet
// cannot promise more than the editor does. A new key in a handler is not forced onto the sheet; add its row by hand.

/** One way to do a thing: a key or a pointer gesture with the modifiers held. `mod` is Ctrl, or Command on a Mac. */
export interface Combo {
  mod?: boolean;
  alt?: boolean;
  shift?: boolean;
  /** `KeyboardEvent.key` for a key (`Escape`, `Enter`, `+`), or the word of a pointer gesture (`Scroll`). */
  key: string;
}

export interface Shortcut {
  id: string;
  name: string;
  keys: Combo[];
}

export interface ShortcutGroup {
  id: string;
  title: string;
  rows: Shortcut[];
}

const k = (key: string): Combo => ({ key });

export const shortcutGroups: ShortcutGroup[] = [
  {
    id: 'tools', title: 'Tools', rows: [
      { id: 'tool-select', name: 'Select', keys: [k('V')] },
      { id: 'tool-frame', name: 'Frame', keys: [k('F')] },
      { id: 'tool-rectangle', name: 'Rectangle', keys: [k('R')] },
      { id: 'tool-text', name: 'Text', keys: [k('T')] },
    ],
  },
  {
    id: 'edit', title: 'Edit', rows: [
      { id: 'undo', name: 'Undo', keys: [{ mod: true, key: 'Z' }] },
      { id: 'redo', name: 'Redo', keys: [{ mod: true, shift: true, key: 'Z' }, { mod: true, key: 'Y' }] },
      { id: 'delete', name: 'Delete the selection', keys: [k('Delete'), k('Backspace')] },
      { id: 'edit-text', name: 'Edit the selected text', keys: [k('Enter')] },
      { id: 'commit-text', name: 'Finish editing the text', keys: [{ mod: true, key: 'Enter' }] },
      { id: 'cancel', name: 'Cancel a drag or a text edit', keys: [k('Escape')] },
    ],
  },
  {
    id: 'view', title: 'View', rows: [
      { id: 'zoom-in', name: 'Zoom in', keys: [k('+'), k('=')] },
      { id: 'zoom-out', name: 'Zoom out', keys: [k('-')] },
      { id: 'fit-all', name: 'Fit all', keys: [{ shift: true, key: '1' }] },
      { id: 'fit-selection', name: 'Zoom to selection', keys: [{ shift: true, key: '2' }] },
      { id: 'scroll', name: 'Pan', keys: [k('Scroll')] },
      { id: 'pan-space', name: 'Pan with the pointer', keys: [k('Space + drag')] },
      { id: 'zoom-wheel', name: 'Zoom at the pointer', keys: [{ mod: true, key: 'Scroll' }] },
    ],
  },
  {
    id: 'selection', title: 'Selection', rows: [
      { id: 'add-select', name: 'Add to or take from the selection', keys: [{ shift: true, key: 'Click' }] },
      { id: 'edit-dblclick', name: 'Edit a text', keys: [k('Double-click')] },
    ],
  },
  {
    id: 'panels', title: 'Panels and help', rows: [
      { id: 'layers-panel', name: 'Layers panel', keys: [{ alt: true, key: '1' }] },
      { id: 'right-panel', name: 'Right panel', keys: [{ alt: true, key: '2' }] },
      { id: 'shortcuts', name: 'Shortcuts', keys: [k('?')] },
    ],
  },
];

const words: Record<string, string> = { Escape: 'Esc' };

/** A combo the way the platform writes it: symbols in front of the key on a Mac (`⇧⌘Z`), words joined by + elsewhere (`Ctrl+Shift+Z`). */
export function comboLabel(c: Combo, mac: boolean): string {
  const key = words[c.key] ?? c.key;
  if (mac) return `${c.alt ? '⌥' : ''}${c.shift ? '⇧' : ''}${c.mod ? '⌘' : ''}${c.alt || c.shift || c.mod ? (key.length === 1 ? '' : ' ') : ''}${key}`;
  return [c.mod ? 'Ctrl' : '', c.alt ? 'Alt' : '', c.shift ? 'Shift' : '', key].filter(Boolean).join('+');
}

/** The ways to do the thing of a row, written for the platform; none for an id the sheet does not have. */
export function shortcutLabels(id: string, mac: boolean): string[] {
  const row = shortcutGroups.flatMap((g) => g.rows).find((r) => r.id === id);
  return row ? row.keys.map((c) => comboLabel(c, mac)) : [];
}

export interface SheetGroup {
  id: string;
  title: string;
  rows: { id: string; name: string; keys: string[] }[];
}

/** The groups of the sheet with every key written for the platform. */
export function sheetGroups(mac: boolean): SheetGroup[] {
  return shortcutGroups.map((g) => ({ id: g.id, title: g.title, rows: g.rows.map((r) => ({ id: r.id, name: r.name, keys: r.keys.map((c) => comboLabel(c, mac)) })) }));
}
