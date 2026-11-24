import { describe, expect, it } from 'vitest';
import { filterEntries, firstEnabled, paletteEntries, paletteKey, type PaletteInput } from './palette';
import { PRESET_ORDER, PRESETS } from './presets';
import { shortcutGroups, shortcutLabels } from './shortcuts';

function setup(over: Partial<Omit<PaletteInput, 'on'>> = {}, withSwitch = false) {
  const calls: unknown[][] = [];
  const log = (name: string) => (...args: unknown[]) => void calls.push([name, ...args]);
  const input: PaletteInput = {
    mac: false, layersOpen: false, inspectorOpen: true, canUndo: true, canRedo: false, canDelete: true, canExport: true, canVariant: true, ...over,
    on: { tool: log('tool'), undo: log('undo'), redo: log('redo'), remove: log('remove'), zoom: log('zoom'), preset: log('preset'), layers: log('layers'), inspector: log('inspector'), exportFrame: log('export'), variant: log('variant'), help: log('help'), ...(withSwitch ? { autoLayout: log('auto-layout') } : {}) },
  };
  return { calls, entries: paletteEntries(input) };
}
const byId = (es: ReturnType<typeof setup>['entries'], id: string) => es.find((e) => e.id === id)!;

describe('the entries of the palette', () => {
  it('lists what the editor has, each id once, each with a name and a group', () => {
    const { entries } = setup();
    expect(entries.map((e) => e.id)).toEqual([
      'tool-select', 'tool-frame', 'tool-rectangle', 'tool-ellipse', 'tool-text',
      'undo', 'redo', 'delete',
      ...PRESET_ORDER.map((p) => `preset-${p}`),
      'zoom-in', 'zoom-out', 'zoom-reset', 'zoom-fit', 'zoom-fit-selection',
      'toggle-layers', 'toggle-inspector',
      'export', 'variant', 'shortcuts',
    ]);
    expect(new Set(entries.map((e) => e.id)).size).toBe(entries.length);
    for (const e of entries) expect([e.label.trim(), e.group.trim()], e.id).not.toContain('');
  });

  it('shows the keys of the shortcut table, written for the platform, and invents none', () => {
    const rows = new Set(shortcutGroups.flatMap((g) => g.rows.map((r) => r.id)));
    for (const mac of [false, true]) {
      const { entries } = setup({ mac });
      for (const e of entries.filter((x) => x.shortcut)) {
        const ids = [...rows].filter((id) => shortcutLabels(id, mac).includes(e.shortcut!));
        expect(ids.length, `${e.id}: ${e.shortcut}`).toBeGreaterThan(0);
      }
      expect(byId(entries, 'undo').shortcut).toBe(mac ? '⌘Z' : 'Ctrl+Z');
      expect(byId(entries, 'redo').shortcut).toBe(mac ? '⇧⌘Z' : 'Ctrl+Shift+Z');
      expect(byId(entries, 'tool-frame').shortcut).toBe('F');
      expect(byId(entries, 'tool-ellipse').shortcut).toBe('E');
      expect(byId(entries, 'tool-ellipse').label).toBe('Ellipse tool');
      expect(byId(entries, 'zoom-fit').shortcut).toBe(mac ? '⇧1' : 'Shift+1');
      expect(byId(entries, 'toggle-layers').shortcut).toBe(mac ? '⌥1' : 'Alt+1');
      expect(byId(entries, 'shortcuts').shortcut).toBe('?');
      expect(byId(entries, 'zoom-reset').shortcut).toBeUndefined();
    }
  });

  it('is enabled only where the editor can do it', () => {
    const on = setup().entries;
    expect(['undo', 'redo', 'delete', 'export', 'variant'].map((id) => byId(on, id).enabled)).toEqual([true, false, true, true, true]);
    const off = setup({ canUndo: false, canRedo: true, canDelete: false, canExport: false, canVariant: false }).entries;
    expect(['undo', 'redo', 'delete', 'export', 'variant'].map((id) => byId(off, id).enabled)).toEqual([false, true, false, false, false]);
    expect(on.filter((e) => !['redo'].includes(e.id)).every((e) => e.enabled)).toBe(true);
  });

  it('names a panel toggle by what it does now', () => {
    const closed = setup({ layersOpen: false, inspectorOpen: false }).entries;
    expect([byId(closed, 'toggle-layers').label, byId(closed, 'toggle-inspector').label]).toEqual(['Show layers panel', 'Show right panel']);
    const open = setup({ layersOpen: true, inspectorOpen: true }).entries;
    expect([byId(open, 'toggle-layers').label, byId(open, 'toggle-inspector').label]).toEqual(['Hide layers panel', 'Hide right panel']);
  });

  it('has one entry for each frame preset, with its name and size', () => {
    const { entries } = setup();
    const frames = entries.filter((e) => e.group === 'Frames');
    expect(frames.map((e) => [e.label, e.detail])).toEqual(PRESET_ORDER.map((p) => [`New frame: ${PRESETS[p].name}`, `${PRESETS[p].w} × ${PRESETS[p].h}`]));
  });

  it('runs the existing handler of the page with what it needs, and nothing else', () => {
    const { entries, calls } = setup();
    for (const id of ['tool-rectangle', 'undo', 'redo', 'delete', 'preset-a4', 'zoom-fit-selection', 'zoom-in', 'toggle-layers', 'toggle-inspector', 'export', 'variant', 'shortcuts']) byId(entries, id).run();
    expect(calls).toEqual([
      ['tool', 'rectangle'], ['undo'], ['redo'], ['remove'], ['preset', 'a4'], ['zoom', 'fit-selection'], ['zoom', 'in'], ['layers'], ['inspector'], ['export'], ['variant'], ['help'],
    ]);
  });

  it('marks the entries that open a dialog, which takes the focus', () => {
    const { entries } = setup();
    expect(entries.filter((e) => e.opensDialog).map((e) => e.id)).toEqual(['export', 'shortcuts']);
  });
});

describe('filtering by what is typed', () => {
  const { entries } = setup();
  const ids = (q: string, es = entries) => filterEntries(es, q).map((e) => e.id);

  it('keeps everything, in order, for an empty or blank query', () => {
    expect(ids('')).toEqual(entries.map((e) => e.id));
    expect(ids('   ')).toEqual(entries.map((e) => e.id));
  });
  it('ignores case and the space around the words', () => {
    expect(ids('  UNDO ')).toEqual(['undo']);
  });
  it('ranks a label that starts with the word, then a word that starts with it, then any match, then a keyword', () => {
    const e = (id: string, label: string, keywords: string[] = []) => ({ id, label, group: 'G', enabled: true, keywords, run: () => {} });
    const list = [e('context', 'Context menu'), e('retext', 'Retext'), e('bold', 'Bold text'), e('tool', 'Text tool'), e('zzz', 'Zzz', ['text'])];
    expect(ids('text', list)).toEqual(['tool', 'bold', 'context', 'retext', 'zzz']);
  });
  it('lists the entry of the same name first for a word of its label', () => {
    expect(ids('fit')).toEqual(['zoom-fit', 'zoom-fit-selection']);
    expect(ids('frame').slice(0, 2)).toEqual(['tool-frame', 'preset-desktop']);
  });
  it('wants every word to match', () => {
    expect(ids('new a4')).toEqual(['preset-a4']);
    expect(ids('zoom selection')).toEqual(['zoom-fit-selection']);
    expect(ids('zoom banana')).toEqual([]);
  });
  it('finds an entry by its size, its group or a keyword', () => {
    expect(ids('1920')).toEqual(['preset-desktopHd']);
    expect(ids('png')).toEqual(['export']);
    expect(ids('keyboard')).toEqual(['shortcuts']);
    expect(ids('circle')).toEqual(['tool-ellipse']);
    expect(ids('oval')).toEqual(['tool-ellipse']);
    expect(ids('panels')).toEqual(['toggle-layers', 'toggle-inspector']);
  });
  it('puts an entry that cannot run after those that can, and keeps it in the list', () => {
    expect(filterEntries(setup({ canUndo: true, canRedo: false }).entries, 'do').map((e) => [e.id, e.enabled])).toEqual([['undo', true], ['redo', false]]);
    expect(filterEntries(setup({ canUndo: false, canRedo: true }).entries, 'do').map((e) => [e.id, e.enabled])).toEqual([['redo', true], ['undo', false]]);
    expect(filterEntries(setup({ canUndo: false, canRedo: false }).entries, 'do').map((e) => e.id)).toEqual(['undo', 'redo']);
  });
  it('treats what is typed as text, not as a pattern', () => {
    expect(ids('.*')).toEqual([]);
    expect(ids('(')).toEqual([]);
  });
});

describe('the keys of the palette', () => {
  const rows = (...enabled: boolean[]) => enabled.map((e) => ({ enabled: e }));
  it('starts on the first entry that can run', () => {
    expect(firstEnabled(rows(false, true, true))).toBe(1);
    expect(firstEnabled(rows(false, false))).toBe(-1);
    expect(firstEnabled([])).toBe(-1);
  });
  it('moves with the arrows, skips what cannot run and wraps round', () => {
    const r = rows(true, false, true);
    expect(paletteKey('ArrowDown', 0, r)).toEqual({ kind: 'move', index: 2 });
    expect(paletteKey('ArrowDown', 2, r)).toEqual({ kind: 'move', index: 0 });
    expect(paletteKey('ArrowUp', 0, r)).toEqual({ kind: 'move', index: 2 });
    expect(paletteKey('ArrowUp', 2, r)).toEqual({ kind: 'move', index: 0 });
  });
  it('runs the active entry with Enter, unless it cannot run', () => {
    expect(paletteKey('Enter', 1, rows(true, true))).toEqual({ kind: 'run', index: 1 });
    expect(paletteKey('Enter', 1, rows(true, false))).toBeNull();
    expect(paletteKey('Enter', -1, rows(true))).toBeNull();
    expect(paletteKey('Enter', 3, rows(true))).toBeNull();
  });
  it('does nothing for an empty list, and leaves every other key to the field', () => {
    expect(paletteKey('ArrowDown', -1, [])).toBeNull();
    expect(paletteKey('Enter', 0, [])).toBeNull();
    expect(paletteKey('ArrowDown', 0, rows(false, false))).toBeNull();
    for (const k of ['Home', 'End', 'a', 'Tab', 'ArrowLeft', 'ArrowRight']) expect(paletteKey(k, 0, rows(true, true))).toBeNull();
  });
});

describe('the entry "Auto layout on canvas"', () => {
  it('is there only when the page has the switch, in the View group, and the list is the one it was without', () => {
    expect(setup().entries.some((e) => e.id === 'auto-layout')).toBe(false);
    const { entries } = setup({}, true);
    const e = byId(entries, 'auto-layout');
    expect([e.label, e.group, e.enabled]).toEqual(['Auto layout on canvas', 'View', true]);
    expect(entries.filter((x) => x.id !== 'auto-layout').map((x) => x.id)).toEqual(setup().entries.map((x) => x.id));
  });
  it('runs the switch of the page and says what is on', () => {
    const on = setup({ autoLayout: true }, true);
    byId(on.entries, 'auto-layout').run();
    expect(on.calls).toEqual([['auto-layout']]);
    expect(byId(on.entries, 'auto-layout').detail).toMatch(/^On/);
    expect(byId(setup({ autoLayout: false }, true).entries, 'auto-layout').detail).toMatch(/^Off/);
  });
  it('is found by a few words of it, and by what it is about', () => {
    const { entries } = setup({}, true);
    for (const q of ['auto layout', 'layout canvas', 'hug', 'switch']) expect(filterEntries(entries, q).some((e) => e.id === 'auto-layout'), q).toBe(true);
    expect(filterEntries(entries, 'auto layout')[0]!.id).toBe('auto-layout');
  });
});
