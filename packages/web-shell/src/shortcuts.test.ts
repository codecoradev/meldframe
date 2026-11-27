import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { wheelAction } from '@meldframe/web-canvas/camera';
import { Interaction, type Env, type Intents, type Mods, type PointerIn } from '@meldframe/web-canvas/interaction';
import { keyAction } from '@meldframe/web-document/edit';
import { isEditKey } from '@meldframe/web-document/textedit';
import { attachInput, type InputHooks } from '@meldframe/web-input/hostinput';
import { layoutKey } from './keys';
import { labels } from './platform';
import { comboLabel, sheetGroups, shortcutGroups, shortcutLabels, type Combo } from './shortcuts';

describe('how a shortcut is written', () => {
  it('uses the symbols on a Mac and words elsewhere, and agrees with the labels of the tool bar', () => {
    for (const mac of [true, false]) {
      const l = labels(mac);
      expect([shortcutLabels('undo', mac)[0], shortcutLabels('redo', mac)[0], shortcutLabels('layers-panel', mac)[0], shortcutLabels('right-panel', mac)[0]]).toEqual([l.undo, l.redo, l.layers, l.inspector]);
    }
    expect(comboLabel({ shift: true, key: '1' }, false)).toBe('Shift+1');
    expect(comboLabel({ shift: true, key: '1' }, true)).toBe('⇧1');
    expect(comboLabel({ mod: true, key: 'Scroll' }, false)).toBe('Ctrl+Scroll');
    expect(comboLabel({ mod: true, key: 'Scroll' }, true)).toBe('⌘ Scroll');
    expect(comboLabel({ key: 'V' }, true)).toBe('V');
  });

  it('answers nothing for a shortcut that is not in the sheet', () => expect(shortcutLabels('nope', false)).toEqual([]));
});

describe('the sheet', () => {
  const rows = shortcutGroups.flatMap((g) => g.rows);

  it('has named groups of named rows, each with a key, and no id twice', () => {
    expect(shortcutGroups.length).toBeGreaterThan(2);
    for (const g of shortcutGroups) {
      expect(g.title.trim()).not.toBe('');
      expect(g.rows.length).toBeGreaterThan(0);
    }
    for (const r of rows) {
      expect(r.name.trim(), r.id).not.toBe('');
      expect(r.keys.length, r.id).toBeGreaterThan(0);
    }
    expect(new Set(rows.map((r) => r.id)).size).toBe(rows.length);
  });

  it('leaves out the row that needs Auto layout on canvas while it is off, and has it while it is on', () => {
    const ids = (on: boolean) => sheetGroups(false, on).flatMap((g) => g.rows.map((r) => r.id));
    expect(ids(true)).toContain('move-in-flow');
    expect(ids(false)).not.toContain('move-in-flow');
    expect(ids(true)).toEqual(sheetGroups(false).flatMap((g) => g.rows.map((r) => r.id)));
    // Every other row is there either way.
    expect(ids(false)).toEqual(ids(true).filter((id) => id !== 'move-in-flow'));
  });

  it('is written for the platform', () => {
    const flat = (mac: boolean) => sheetGroups(mac).flatMap((g) => g.rows.flatMap((r) => r.keys));
    expect(flat(false)).toContain('Ctrl+Z');
    expect(flat(true)).toContain('⌘Z');
    expect(flat(true)).not.toContain('Ctrl+Z');
  });
});

// The sheet may only list what the editor answers to. Each row has a check against the code that answers: the same
// events the keys of the row describe are fed to the real handlers. A row without a check, or a check without a row, fails.
const NONE: Mods = { shift: false, alt: false, ctrl: false, meta: false };
const eventOf = (c: Combo) => ({
  key: c.key,
  code: /^\d$/.test(c.key) ? `Digit${c.key}` : /^[a-z]$/i.test(c.key) ? `Key${c.key.toUpperCase()}` : c.key,
  ctrlKey: !!c.mod, metaKey: false, altKey: !!c.alt, shiftKey: !!c.shift,
});

function rig() {
  const log: unknown[][] = [];
  const state = { camera: { x: 0, y: 0, zoom: 1 }, selection: ['a'] as string[] };
  const env: Env = {
    camera: () => state.camera, selection: () => state.selection, tool: () => 'move', viewport: () => ({ w: 800, h: 600 }),
    hit: (x, y) => (x >= 0 && x <= 100 && y >= 0 && y <= 100 ? 'a' : x >= 200 && x <= 300 && y >= 0 && y <= 100 ? 'b' : null),
    selectionBox: () => null,
  };
  const out: Intents = {
    select: (ids, additive) => log.push(['select', ids, additive]), move: (i) => log.push(['move', i.final, i.cancelled]),
    resize: () => {}, create: () => {}, hover: () => {}, fit: (k) => log.push(['fit', k]),
    camera: (c, final) => { state.camera = c; log.push(['camera', c, final]); },
  };
  const it = new Interaction(env, out);
  const key = (c: Combo, down = true) => { const e = eventOf(c); return it.key({ key: e.key, code: e.code, down, mods: { shift: e.shiftKey, alt: e.altKey, ctrl: e.ctrlKey, meta: e.metaKey } }); };
  return { it, log, state, key };
}
const point = (x: number, y: number, mods: Mods = NONE): PointerIn => ({ id: 1, x, y, button: 0, pointerType: 'mouse', mods });
const zoomOf = (r: ReturnType<typeof rig>) => (r.log.find((e) => e[0] === 'camera')![1] as { zoom: number }).zoom;
const viewport = { w: 800, h: 600 };
const editor = () => readFileSync(new URL('../../../apps/web/src/TextEditor.svelte', import.meta.url), 'utf8');

const checks: Record<string, (c: Combo) => void> = {
  ...Object.fromEntries((['select', 'frame', 'rectangle', 'ellipse', 'text'] as const).map((tool) => [`tool-${tool}`, (c: Combo) => expect(keyAction(eventOf(c), [])).toEqual({ type: 'setTool', tool })])),
  undo: (c) => expect(keyAction(eventOf(c), [])).toEqual({ type: 'undo' }),
  redo: (c) => expect(keyAction(eventOf(c), [])).toEqual({ type: 'redo' }),
  delete: (c) => expect(keyAction(eventOf(c), ['a'])).toEqual({ type: 'delete', ids: ['a'] }),
  'move-in-flow': (c) => expect(keyAction(eventOf(c), ['a'])).toEqual({ type: 'arrowKey', direction: c.key.slice('Arrow'.length).toLowerCase() }),
  'edit-text': (c) => expect(isEditKey(eventOf(c))).toBe(true),
  // The field of `apps/web/src/TextEditor.svelte` is a view with no pure part: its keys are read from its source.
  'commit-text': () => expect(editor()).toMatch(/e\.key === 'Enter' && \(e\.ctrlKey \|\| e\.metaKey\)/),
  cancel: (c) => {
    const r = rig();
    r.it.down(point(50, 50));
    r.it.move(point(90, 90));
    expect(r.key(c)).toBe(true);
    expect(r.log).toContainEqual(['move', true, true]);
    expect(editor()).toMatch(/e\.key === 'Escape'/);
  },
  'zoom-in': (c) => { const r = rig(); expect(r.key(c)).toBe(true); expect(zoomOf(r)).toBeGreaterThan(1); },
  'zoom-out': (c) => { const r = rig(); expect(r.key(c)).toBe(true); expect(zoomOf(r)).toBeLessThan(1); },
  'fit-all': (c) => { const r = rig(); expect(r.key(c)).toBe(true); expect(r.log).toContainEqual(['fit', 'all']); },
  'fit-selection': (c) => { const r = rig(); expect(r.key(c)).toBe(true); expect(r.log).toContainEqual(['fit', 'selection']); },
  scroll: () => expect(wheelAction({ deltaX: 0, deltaY: 30, deltaMode: 0, ctrlKey: false, metaKey: false }, viewport).kind).toBe('pan'),
  'pan-space': () => {
    const r = rig();
    r.key({ key: ' ' });
    r.it.down(point(50, 50));
    r.it.move(point(80, 70));
    expect(r.log.some((e) => e[0] === 'camera')).toBe(true);
    expect(r.log.some((e) => e[0] === 'select' || e[0] === 'move')).toBe(false);
  },
  'zoom-wheel': () => {
    for (const k of [{ ctrlKey: true, metaKey: false }, { ctrlKey: false, metaKey: true }]) {
      expect(wheelAction({ deltaX: 0, deltaY: -30, deltaMode: 0, ...k }, viewport).kind).toBe('zoom');
    }
  },
  'add-select': () => {
    const r = rig();
    r.it.down(point(250, 50, { ...NONE, shift: true }));
    r.it.up(point(250, 50));
    expect(r.log).toEqual([['select', ['b'], true]]);
  },
  'edit-dblclick': () => {
    const host = Object.assign(new EventTarget(), { style: { cursor: '' }, getBoundingClientRect: () => ({ left: 0, top: 0 }), focus: () => {}, setPointerCapture: () => {} });
    const seen: string[] = [];
    const hooks = {
      interaction: {} as InputHooks['interaction'], tool: () => 'move', camera: () => ({ x: 0, y: 0, zoom: 1 }), hit: () => 'n1',
      schedule: () => {}, dropDraft: () => {}, settleSoon: () => {}, activate: (id: string) => seen.push(id),
    } as InputHooks;
    attachInput(host, hooks);
    host.dispatchEvent(Object.assign(new Event('dblclick'), { clientX: 5, clientY: 5 }));
    expect(seen).toEqual(['n1']);
  },
  'layers-panel': (c) => expect(layoutKey(eventOf(c))).toBe('layers'),
  'right-panel': (c) => expect(layoutKey(eventOf(c))).toBe('inspector'),
  shortcuts: (c) => expect(layoutKey(eventOf(c))).toBe('help'),
  palette: (c) => expect(layoutKey(eventOf(c))).toBe('palette'),
};

describe('every row of the sheet is something the editor does', () => {
  it('has a check for each row and a row for each check', () => {
    expect(Object.keys(checks).sort()).toEqual(shortcutGroups.flatMap((g) => g.rows.map((r) => r.id)).sort());
  });
  for (const row of shortcutGroups.flatMap((g) => g.rows)) {
    it(`${row.name}: ${row.keys.map((k) => comboLabel(k, false)).join(', ')}`, () => {
      for (const k of row.keys) checks[row.id]!(k);
    });
  }
});
