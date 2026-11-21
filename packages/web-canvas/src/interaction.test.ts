import { describe, expect, it } from 'vitest';
import type { Camera } from './camera';
import { screenToWorld } from './camera';
import type { Rect, Size } from './geometry';
import {
  DRAG_THRESHOLD, Interaction, type Env, type Intents, type Mods, type MoveIntent, type PointerIn, type ToolMode,
} from './interaction';

const NONE: Mods = { shift: false, alt: false, ctrl: false, meta: false };

interface Rig {
  it: Interaction;
  log: unknown[][];
  /** The move intents as they were reported. */
  moves: MoveIntent[];
  state: { camera: Camera; selection: string[]; tool: ToolMode; box: Rect | null };
}

/** Two nodes in world space: "a" at (0,0) 100x100 and "b" at (200,0) 100x100. */
function rig(init: Partial<Rig['state']> = {}): Rig {
  const state = { camera: { x: 0, y: 0, zoom: 1 } as Camera, selection: [] as string[], tool: 'move' as ToolMode, box: null as Rect | null, ...init };
  const log: unknown[][] = [];
  const moves: MoveIntent[] = [];
  const viewport: Size = { w: 800, h: 600 };
  const env: Env = {
    camera: () => state.camera,
    selection: () => state.selection,
    tool: () => state.tool,
    viewport: () => viewport,
    hit: (x, y) => (x >= 0 && x <= 100 && y >= 0 && y <= 100 ? 'a' : x >= 200 && x <= 300 && y >= 0 && y <= 100 ? 'b' : null),
    selectionBox: () => state.box,
  };
  const out: Intents = {
    select: (ids, additive) => log.push(['select', ids, additive]),
    move: (i) => {
      moves.push(i);
      log.push(['move', i.ids, i.dx, i.dy, i.final, i.cancelled]);
    },
    resize: (i) => log.push(['resize', i.ids, i.handle, i.dx, i.dy, i.final, i.cancelled]),
    create: (i) => log.push(['create', i.tool, i.at, i.x, i.y, i.w, i.h, i.final, i.cancelled]),
    camera: (c, final) => { state.camera = c; log.push(['camera', c, final]); },
    hover: (id) => log.push(['hover', id]),
    fit: (kind) => log.push(['fit', kind]),
  };
  return { it: new Interaction(env, out), log, moves, state };
}
const p = (x: number, y: number, o: Partial<PointerIn> = {}): PointerIn => ({ id: 1, x, y, button: 0, pointerType: 'mouse', mods: NONE, ...o });
const kinds = (r: Rig) => r.log.map((e) => e[0]);

describe('selection clicks', () => {
  it('a click on a node selects it', () => {
    const r = rig();
    r.it.down(p(50, 50));
    r.it.up(p(50, 50));
    expect(r.log).toEqual([['select', ['a'], false]]);
  });

  it('a click on empty canvas clears the selection', () => {
    const r = rig({ selection: ['a'] });
    r.it.down(p(500, 500));
    r.it.up(p(500, 500));
    expect(r.log).toEqual([['select', [], false]]);
  });

  it('shift adds without clearing, also on empty canvas', () => {
    const r = rig({ selection: ['a'] });
    r.it.down(p(250, 50, { mods: { ...NONE, shift: true } }));
    r.it.up(p(250, 50));
    expect(r.log).toEqual([['select', ['b'], true]]);
    const e = rig({ selection: ['a'] });
    e.it.down(p(500, 500, { mods: { ...NONE, shift: true } }));
    e.it.up(p(500, 500));
    expect(e.log).toEqual([]);
  });

  it('clicking one of several selected nodes narrows to it on release, but not when it was dragged', () => {
    const r = rig({ selection: ['a', 'b'] });
    r.it.down(p(50, 50));
    expect(r.log).toEqual([]);
    r.it.up(p(50, 50));
    expect(r.log).toEqual([['select', ['a'], false]]);
    const d = rig({ selection: ['a', 'b'] });
    d.it.down(p(50, 50));
    d.it.move(p(80, 50));
    d.it.up(p(80, 50));
    expect(kinds(d)).not.toContain('select');
  });

  it('shift-click on a selected node reports the additive click for the host to toggle', () => {
    const r = rig({ selection: ['a'] });
    r.it.down(p(50, 50, { mods: { ...NONE, shift: true } }));
    r.it.up(p(50, 50));
    expect(r.log).toEqual([['select', ['a'], true]]);
  });

  it('other buttons do nothing', () => {
    const r = rig();
    r.it.down(p(50, 50, { button: 2 }));
    r.it.up(p(50, 50, { button: 2 }));
    expect(r.log).toEqual([]);
  });
});

describe('move', () => {
  it('a drag selects the node, then reports world deltas and a final one on release', () => {
    const r = rig({ camera: { x: 0, y: 0, zoom: 2 } });
    r.it.down(p(50, 50));
    r.it.move(p(52, 50)); // below the threshold: nothing yet
    expect(r.log).toEqual([['select', ['a'], false]]);
    r.it.move(p(70, 90));
    r.it.up(p(70, 90));
    expect(r.log.slice(1)).toEqual([
      ['move', ['a'], 10, 20, false, false],
      ['move', ['a'], 10, 20, true, false],
    ]);
  });

  it('deltas are totals since the press, so a host that applies them gets no drift', () => {
    const r = rig();
    r.it.down(p(10, 10));
    r.it.move(p(20, 10));
    r.it.move(p(40, 30));
    const moves = r.log.filter((e) => e[0] === 'move');
    expect(moves.map((m) => [m[2], m[3]])).toEqual([[10, 0], [30, 20]]);
  });

  it('a drag of a selected node moves the whole selection', () => {
    const r = rig({ selection: ['a', 'b'] });
    r.it.down(p(50, 50));
    r.it.move(p(60, 50));
    expect(r.log[0]).toEqual(['move', ['a', 'b'], 10, 0, false, false]);
  });

  it('Escape cancels: a final cancelled intent with no delta', () => {
    const r = rig();
    r.it.down(p(50, 50));
    r.it.move(p(90, 50));
    r.it.key({ key: 'Escape', down: true, mods: NONE });
    r.it.up(p(90, 50));
    expect(r.log.at(-1)).toEqual(['move', ['a'], 0, 0, true, true]);
    expect(r.log.filter((e) => e[0] === 'move' && e[4] === true)).toHaveLength(1);
  });

  it('a cancelled pointer (the system took it) ends the gesture the same way', () => {
    const r = rig();
    r.it.down(p(50, 50));
    r.it.move(p(90, 50));
    r.it.cancel();
    expect(r.log.at(-1)).toEqual(['move', ['a'], 0, 0, true, true]);
  });

  it('the threshold is in screen pixels', () => {
    const r = rig({ camera: { x: 0, y: 0, zoom: 0.1 } });
    r.it.down(p(5, 5));
    r.it.move(p(5 + DRAG_THRESHOLD - 1, 5));
    expect(kinds(r)).toEqual(['select']);
    r.it.move(p(5 + DRAG_THRESHOLD + 1, 5));
    expect(kinds(r)).toEqual(['select', 'move']);
  });
});

describe('move reports where the pointer is', () => {
  it('the world position of the pointer comes with every move intent, while dragging and on release', () => {
    const r = rig();
    r.it.down(p(50, 50));
    r.it.move(p(70, 90));
    r.it.up(p(75, 95));
    expect(r.moves.map((m) => [m.at, m.final])).toEqual([[{ x: 70, y: 90 }, false], [{ x: 75, y: 95 }, true]]);
  });
  it('is in world units, not screen pixels, when the camera is zoomed and panned', () => {
    const r = rig({ camera: { x: 100, y: 20, zoom: 2 } });
    r.it.down(p(150, 70)); // world (25, 25), on node a
    r.it.move(p(200, 120));
    r.it.up(p(220, 140));
    const last = r.moves.at(-1)!;
    expect(last.at).toEqual(screenToWorld({ x: 100, y: 20, zoom: 2 }, 220, 140));
    expect([last.dx, last.dy]).toEqual([35, 35]);
  });
  it('a cancelled drag still says where the pointer was pressed', () => {
    const r = rig();
    r.it.down(p(50, 50));
    r.it.move(p(90, 50));
    r.it.cancel();
    expect(r.moves.at(-1)).toMatchObject({ cancelled: true, at: { x: 50, y: 50 } });
  });
});

describe('move with shift', () => {
  it('does not list a node twice when the host already added it to the selection on press', () => {
    const r = rig({ selection: ['b'] });
    r.it.down(p(50, 50, { mods: { ...NONE, shift: true } }));
    r.state.selection = ['b', 'a']; // the host applied select(['a'], true) after the press
    r.it.move(p(80, 50, { mods: { ...NONE, shift: true } }));
    expect(r.log.find((e) => e[0] === 'move')?.[1]).toEqual(['b', 'a']);
  });
});

describe('space pan', () => {
  it('cancel clears a held Space whose key-up was lost (window blur), so clicks select again', () => {
    const r = rig();
    r.it.key({ key: ' ', code: 'Space', down: true, mods: NONE });
    r.it.cancel();
    r.it.down(p(50, 50));
    r.it.up(p(50, 50));
    expect(r.log).toEqual([['select', ['a'], false]]);
  });
});

describe('resize', () => {
  const box: Rect = { x: 0, y: 0, w: 100, h: 100 };
  it('a press on a handle resizes the selection, not selects', () => {
    const r = rig({ selection: ['a'], box, camera: { x: 0, y: 0, zoom: 2 } });
    r.it.down(p(100, 100)); // the se handle of the screen box
    r.it.move(p(120, 110));
    r.it.up(p(120, 110));
    expect(r.log).toEqual([
      ['resize', ['a'], 'se', 10, 5, false, false],
      ['resize', ['a'], 'se', 10, 5, true, false],
    ]);
  });

  it('a resize released back near its start still ends with a final intent', () => {
    const r = rig({ selection: ['a'], box });
    r.it.down(p(100, 100));
    r.it.move(p(120, 120));
    r.it.move(p(101, 100)); // back inside the drag threshold after the resize began
    r.it.up(p(101, 100));
    expect(r.log).toEqual([
      ['resize', ['a'], 'se', 20, 20, false, false],
      ['resize', ['a'], 'se', 1, 0, false, false],
      ['resize', ['a'], 'se', 1, 0, true, false],
    ]);
  });

  it('a press on a handle that never leaves the threshold emits nothing', () => {
    const r = rig({ selection: ['a'], box });
    r.it.down(p(100, 100));
    r.it.up(p(101, 100));
    expect(r.log).toEqual([]);
  });

  it('hovering a handle sets the resize cursor', () => {
    const r = rig({ selection: ['a'], box });
    r.it.move(p(0, 0, { id: 7 }));
    expect(r.it.cursor).toBe('nwse-resize');
    r.it.move(p(50, 50, { id: 7 }));
    expect(r.it.cursor).toBe('default');
  });

  it('Escape cancels a resize', () => {
    const r = rig({ selection: ['a'], box });
    r.it.down(p(100, 50));
    r.it.move(p(130, 50));
    r.it.key({ key: 'Escape', down: true, mods: NONE });
    expect(r.log.at(-1)).toEqual(['resize', ['a'], 'e', 0, 0, true, true]);
  });
});

describe('camera gestures', () => {
  it('the hand tool pans with the left button, reporting final on release', () => {
    const r = rig({ tool: 'hand' });
    r.it.down(p(100, 100));
    r.it.move(p(130, 90));
    r.it.up(p(130, 90));
    expect(r.log).toEqual([
      ['camera', { x: 30, y: -10, zoom: 1 }, false],
      ['camera', { x: 30, y: -10, zoom: 1 }, true],
    ]);
    expect(kinds(r)).not.toContain('select');
  });

  it('the middle button pans in any tool', () => {
    const r = rig();
    r.it.down(p(0, 0, { button: 1 }));
    r.it.move(p(10, 10));
    r.it.up(p(10, 10));
    expect(r.state.camera).toEqual({ x: 10, y: 10, zoom: 1 });
  });

  it('holding Space pans with the left button and shows a grab cursor; releasing Space ends it', () => {
    const r = rig();
    r.it.key({ key: ' ', down: true, mods: NONE });
    expect(r.it.cursor).toBe('grab');
    r.it.down(p(0, 0));
    expect(r.it.cursor).toBe('grabbing');
    r.it.move(p(5, 0));
    r.it.up(p(5, 0));
    expect(r.state.camera.x).toBe(5);
    r.it.key({ key: ' ', down: false, mods: NONE });
    expect(r.it.cursor).toBe('default');
  });

  it('a wheel pans, ctrl+wheel zooms around the cursor', () => {
    const r = rig();
    r.it.wheel({ x: 200, y: 100, deltaX: 0, deltaY: 30, deltaMode: 0, ctrlKey: false, metaKey: false });
    expect(r.state.camera).toEqual({ x: 0, y: -30, zoom: 1 });
    const before = screenToWorld(r.state.camera, 200, 100);
    r.it.wheel({ x: 200, y: 100, deltaX: 0, deltaY: -20, deltaMode: 0, ctrlKey: true, metaKey: false });
    const after = screenToWorld(r.state.camera, 200, 100);
    expect(after.x).toBeCloseTo(before.x);
    expect(after.y).toBeCloseTo(before.y);
    expect(r.state.camera.zoom).toBeGreaterThan(1);
    expect(r.log.every((e) => e[0] === 'camera' && e[2] === false)).toBe(true);
  });

  it('two fingers pinch and pan, cancel a drag in progress, and the leftover finger is ignored', () => {
    const r = rig();
    r.it.down(p(50, 50, { id: 1, pointerType: 'touch' }));
    r.it.move(p(60, 50, { id: 1, pointerType: 'touch' }));
    r.it.down(p(300, 300, { id: 2, pointerType: 'touch' }));
    expect(r.log.at(-1)).toEqual(['move', ['a'], 0, 0, true, true]);
    r.it.move(p(40, 50, { id: 1, pointerType: 'touch' })); // fingers 60,50 / 300,300 -> 40,50 / 320,300
    r.it.move(p(320, 300, { id: 2, pointerType: 'touch' }));
    expect(r.state.camera.zoom).toBeGreaterThan(1);
    r.it.up(p(320, 300, { id: 2, pointerType: 'touch' }));
    expect(r.log.at(-1)).toEqual(['camera', r.state.camera, true]);
    const n = r.log.length;
    r.it.move(p(10, 10, { id: 1, pointerType: 'touch' }));
    r.it.up(p(10, 10, { id: 1, pointerType: 'touch' }));
    expect(r.log.length).toBe(n); // the remaining finger neither moves a node nor pans
  });

  it('plus, minus and shift+1 / shift+2 are camera shortcuts', () => {
    const r = rig();
    expect(r.it.key({ key: '+', down: true, mods: NONE })).toBe(true);
    expect(r.state.camera.zoom).toBeGreaterThan(1);
    // the viewport centre stays put
    const w = screenToWorld(r.state.camera, 400, 300);
    expect(w.x).toBeCloseTo(400);
    r.it.key({ key: '-', down: true, mods: NONE });
    expect(r.state.camera.zoom).toBeCloseTo(1);
    r.it.key({ key: '1', down: true, mods: { ...NONE, shift: true } });
    r.it.key({ key: '2', down: true, mods: { ...NONE, shift: true } });
    expect(r.log.filter((e) => e[0] === 'fit')).toEqual([['fit', 'all'], ['fit', 'selection']]);
    expect(r.it.key({ key: 'q', down: true, mods: NONE })).toBe(false);
  });
});

describe('create tools', () => {
  it('a drag reports a normalised rectangle in world units and the node under the start', () => {
    const r = rig({ tool: 'rect', camera: { x: 100, y: 0, zoom: 2 } });
    r.it.down(p(150, 100)); // world (25, 50)
    r.it.move(p(110, 60)); // world (5, 30): dragging up-left
    r.it.up(p(110, 60));
    expect(r.log).toEqual([
      ['create', 'rect', 'a', 5, 30, 20, 20, false, false],
      ['create', 'rect', 'a', 5, 30, 20, 20, true, false],
    ]);
  });

  it('a click without a drag asks for a default-sized node: a final intent of size 0', () => {
    const r = rig({ tool: 'frame' });
    r.it.down(p(500, 500));
    r.it.up(p(500, 500));
    expect(r.log).toEqual([['create', 'frame', null, 500, 500, 0, 0, true, false]]);
  });

  it('Escape cancels', () => {
    const r = rig({ tool: 'text' });
    r.it.down(p(500, 500));
    r.it.move(p(550, 540));
    r.it.key({ key: 'Escape', down: true, mods: NONE });
    expect(r.log.at(-1)).toEqual(['create', 'text', null, 500, 500, 0, 0, true, true]);
  });
});

describe('hover', () => {
  it('reports changes only and clears when the pointer leaves', () => {
    const r = rig();
    r.it.move(p(50, 50));
    r.it.move(p(60, 60));
    r.it.move(p(250, 50));
    r.it.move(p(500, 500));
    r.it.move(p(510, 500));
    r.it.leave();
    expect(r.log).toEqual([['hover', 'a'], ['hover', 'b'], ['hover', null]]);
  });

  it('is silent while a button is held', () => {
    const r = rig();
    r.it.down(p(50, 50));
    r.it.move(p(250, 50));
    expect(kinds(r)).not.toContain('hover');
  });
});
