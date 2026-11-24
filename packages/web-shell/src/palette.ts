// The command palette: the entries it lists, how a typed query narrows them, and what the arrow keys and Enter do. Pure.
// An entry is data (id, name, group, keys, whether it can run now) plus a handler of the page: all but Delete are the
// ones the tool bar, the menus and the keys already call, and Delete is built from the same `deleteAction` rule as the
// Delete key. Running one calls it, so the palette decides nothing about the document. The keys shown come from the
// shortcut table, never from here; a thing with no key has none.
import { nextIndex } from '@meldframe/web-chrome/focus';
import { PRESET_ORDER, PRESETS, sizeText } from './presets';
import type { FramePreset } from '@meldframe/protocol';
import { shortcutLabels } from './shortcuts';
import { zoomItems, type ZoomCommand } from './zoom';

type Tool = 'select' | 'frame' | 'rectangle' | 'ellipse' | 'text';

export interface PaletteEntry {
  id: string;
  label: string;
  group: string;
  /** The first key the shortcut table lists for it, written for the platform. */
  shortcut?: string;
  /** More text for the row, as the size of a preset; it is searched too. */
  detail?: string;
  /** False while the editor cannot do it (nothing to undo, nothing selected). */
  enabled: boolean;
  /** Words that find it besides its label. */
  keywords: readonly string[];
  /** It opens a dialog, which takes the focus: the palette does not give it back. */
  opensDialog?: boolean;
  run: () => void;
}

export interface PaletteInput {
  mac: boolean;
  layersOpen: boolean;
  inspectorOpen: boolean;
  canUndo: boolean;
  canRedo: boolean;
  canDelete: boolean;
  canExport: boolean;
  canVariant: boolean;
  /** "Auto layout on canvas" is on. */
  autoLayout?: boolean;
  /** The handlers of the page: the ones the tool bar, the menus and the keys already call. */
  on: {
    tool: (t: Tool) => void;
    undo: () => void;
    redo: () => void;
    remove: () => void;
    zoom: (c: ZoomCommand) => void;
    preset: (p: FramePreset) => void;
    layers: () => void;
    inspector: () => void;
    exportFrame: () => void;
    variant: () => void;
    help: () => void;
    /** Turns "Auto layout on canvas" on or off; with none the entry is left out (a page without the switch). */
    autoLayout?: () => void;
  };
}

const tools: { tool: Tool; label: string; words?: string[] }[] = [
  { tool: 'select', label: 'Select tool' }, { tool: 'frame', label: 'Frame tool' }, { tool: 'rectangle', label: 'Rectangle tool' }, { tool: 'ellipse', label: 'Ellipse tool', words: ['circle', 'oval'] }, { tool: 'text', label: 'Text tool' },
];
// Words besides the label, for the zoom rows (their labels come from the zoom menu).
const zoomWords: Record<ZoomCommand, string[]> = { in: ['closer'], out: ['further'], reset: ['reset', 'actual size'], fit: ['fit', 'everything'], 'fit-selection': ['fit', 'focus'] };

/** Every entry the editor has today, in the order the palette lists them when nothing is typed. */
export function paletteEntries(i: PaletteInput): PaletteEntry[] {
  const key = (id: string) => shortcutLabels(id, i.mac)[0];
  const { on } = i;
  const entry = (group: string, id: string, label: string, enabled: boolean, run: () => void, more: Partial<PaletteEntry> = {}): PaletteEntry => ({ id, label, group, enabled, keywords: [], run, ...more });
  return [
    ...tools.map((t) => entry('Tools', `tool-${t.tool}`, t.label, true, () => on.tool(t.tool), { shortcut: key(`tool-${t.tool}`), keywords: t.words ?? [] })),
    entry('Edit', 'undo', 'Undo', i.canUndo, on.undo, { shortcut: key('undo') }),
    entry('Edit', 'redo', 'Redo', i.canRedo, on.redo, { shortcut: key('redo') }),
    entry('Edit', 'delete', 'Delete the selection', i.canDelete, on.remove, { shortcut: key('delete'), keywords: ['remove'] }),
    ...PRESET_ORDER.map((p) => entry('Frames', `preset-${p}`, `New frame: ${PRESETS[p].name}`, true, () => on.preset(p), { detail: sizeText(p), keywords: ['preset', 'create', 'add'] })),
    ...zoomItems({ mac: i.mac, onZoom: on.zoom }).map((z) => entry('View', `zoom-${z.id}`, z.label, true, z.onSelect, { shortcut: z.shortcut, keywords: zoomWords[z.id] })),
    ...(on.autoLayout ? [entry('View', 'auto-layout', 'Auto layout on canvas', true, on.autoLayout, { detail: i.autoLayout ? 'On: choose to turn off' : 'Off: choose to turn on', keywords: ['layout', 'flow', 'hug', 'fill', 'toggle', 'switch'] })] : []),
    entry('Panels', 'toggle-layers', `${i.layersOpen ? 'Hide' : 'Show'} layers panel`, true, on.layers, { shortcut: key('layers-panel'), keywords: ['pages', 'toggle'] }),
    entry('Panels', 'toggle-inspector', `${i.inspectorOpen ? 'Hide' : 'Show'} right panel`, true, on.inspector, { shortcut: key('right-panel'), keywords: ['design', 'inspector', 'properties', 'toggle'] }),
    entry('File', 'export', 'Export…', i.canExport, on.exportFrame, { keywords: ['png', 'html', 'tailwind', 'image'], opensDialog: true }),
    entry('File', 'variant', 'Make a variant', i.canVariant, on.variant, { keywords: ['copy', 'duplicate'] }),
    entry('Help', 'shortcuts', 'Show keyboard shortcuts', true, on.help, { shortcut: key('shortcuts'), keywords: ['keys', 'help'], opensDialog: true }),
  ];
}

/** How well the words typed match an entry, or null when one of them matches nothing: a label that starts with the word beats a word of it that does, then any part, then the group, a keyword or the detail. */
function score(e: PaletteEntry, words: readonly string[]): number | null {
  const label = e.label.toLowerCase();
  const labelWords = label.split(/[^a-z0-9]+/);
  const rest = [e.group, e.detail ?? '', ...e.keywords].map((s) => s.toLowerCase());
  let total = 0;
  for (const w of words) {
    const s = label.startsWith(w) ? 4 : labelWords.some((x) => x.startsWith(w)) ? 3 : label.includes(w) ? 2 : rest.some((x) => x.includes(w)) ? 1 : 0;
    if (s === 0) return null;
    total += s;
  }
  return total;
}

/**
 * The entries a query leaves: every word of it must match (case does not matter, and it is text, not a pattern). The
 * best match is first and an entry that cannot run comes after those that can; equals keep the order of the list. A blank
 * query keeps the list as it is.
 */
export function filterEntries<T extends PaletteEntry>(entries: readonly T[], query: string): T[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) return [...entries];
  return entries
    .map((e, at) => ({ e, at, s: score(e, words) }))
    .filter((x): x is { e: T; at: number; s: number } => x.s !== null)
    .sort((a, b) => Number(b.e.enabled) - Number(a.e.enabled) || b.s - a.s || a.at - b.at)
    .map((x) => x.e);
}

/** The first entry that can run, where the highlight starts; -1 when none can. */
export const firstEnabled = (entries: readonly { enabled: boolean }[]): number => entries.findIndex((e) => e.enabled);

export type PaletteKey = { kind: 'move'; index: number } | { kind: 'run'; index: number } | null;

/** The up and down arrows move the highlight over the entries that can run, round the ends; Enter runs the highlighted one; every other key is the field's. */
export function paletteKey(key: string, active: number, entries: readonly { enabled: boolean }[]): PaletteKey {
  if (key === 'Enter') return entries[active]?.enabled ? { kind: 'run', index: active } : null;
  if (key !== 'ArrowDown' && key !== 'ArrowUp') return null;
  const to = nextIndex(key, active, entries.length, entries.map((e) => !e.enabled));
  return to === undefined ? null : { kind: 'move', index: to };
}
