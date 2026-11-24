import { describe, expect, it } from 'vitest';
import { captureBounds, captureRectOf, contentBounds, hitTest, rectsOf, resolveScene, type Scene, type SceneNode } from './scene';

const node = (o: Partial<SceneNode> & Pick<SceneNode, 'id'>): SceneNode => ({
  parent: null, kind: 'rect', x: 0, y: 0, w: 10, h: 10, fill: 1, ...o,
});
const parentScene = (...nodes: SceneNode[]): Scene => ({ space: 'parent', nodes });
const worldScene = (...nodes: SceneNode[]): Scene => ({ space: 'world', nodes });

describe('resolveScene', () => {
  it('resolves the weight and the alignment of a text, regular and left when the scene says nothing', () => {
    const rs = resolveScene(parentScene(
      node({ id: 'a', kind: 'text', fontWeight: 700, textAlign: 'right' }),
      node({ id: 'b', kind: 'text' }),
    )).scene;
    expect(rs.byId.get('a')).toMatchObject({ fontWeight: 700, textAlign: 'right' });
    expect(rs.byId.get('b')).toMatchObject({ fontWeight: 400, textAlign: 'left' });
  });

  it('a change of weight or alignment is a change of the node, so the text is drawn again', () => {
    const first = resolveScene(parentScene(node({ id: 'a', kind: 'text', fontWeight: 400 }))).scene;
    expect(resolveScene(parentScene(node({ id: 'a', kind: 'text', fontWeight: 700 })), first).patch.changed).toEqual(['a']);
    expect(resolveScene(parentScene(node({ id: 'a', kind: 'text', fontWeight: 400, textAlign: 'center' })), first).patch.changed).toEqual(['a']);
    expect(resolveScene(parentScene(node({ id: 'a', kind: 'text', fontWeight: 400 })), first).patch.changed).toEqual([]);
  });

  it('resolves the alpha of a fill, opaque when the scene says nothing, and a change of it is a change of the node', () => {
    const rs = resolveScene(parentScene(node({ id: 'a', fillAlpha: 0.5 }), node({ id: 'b' }))).scene;
    expect(rs.byId.get('a')?.fillAlpha).toBe(0.5);
    expect(rs.byId.get('b')?.fillAlpha).toBe(1);
    // A fully transparent fill (`#00000000`) is alpha 0, not "none given".
    expect(resolveScene(parentScene(node({ id: 'z', fillAlpha: 0 }))).scene.byId.get('z')?.fillAlpha).toBe(0);
    const first = resolveScene(parentScene(node({ id: 'a', fillAlpha: 0.5 }))).scene;
    expect(resolveScene(parentScene(node({ id: 'a', fillAlpha: 0.25 })), first).patch.changed).toEqual(['a']);
    expect(resolveScene(parentScene(node({ id: 'a', fillAlpha: 0.5 })), first).patch.changed).toEqual([]);
  });

  it('resolves the border of a node, none when the scene says nothing, and a change of it is a change of the node', () => {
    const border = { width: 4, color: 0xff0000, alpha: 1, position: 'outside' as const };
    const rs = resolveScene(parentScene(node({ id: 'a', border }), node({ id: 'b' }))).scene;
    expect(rs.byId.get('a')?.border).toEqual(border);
    expect(rs.byId.get('b')?.border).toBeUndefined();
    const first = resolveScene(parentScene(node({ id: 'a', border }))).scene;
    expect(resolveScene(parentScene(node({ id: 'a', border: { ...border, width: 5 } })), first).patch.changed).toEqual(['a']);
    expect(resolveScene(parentScene(node({ id: 'a', border: { ...border } })), first).patch.changed).toEqual([]);
    expect(resolveScene(parentScene(node({ id: 'a' })), first).patch.changed).toEqual(['a']);
  });

  it('resolves the shadows of a node, none when the scene says nothing, and a change of them is a change of the node', () => {
    const shadows = [{ x: 0, y: 4, blur: 8, spread: 0, color: 0, alpha: 0.25 }];
    const rs = resolveScene(parentScene(node({ id: 'a', shadows }), node({ id: 'b' }))).scene;
    expect(rs.byId.get('a')?.shadows).toEqual(shadows);
    expect(rs.byId.get('b')?.shadows).toBeUndefined();
    const first = resolveScene(parentScene(node({ id: 'a', shadows }))).scene;
    expect(resolveScene(parentScene(node({ id: 'a', shadows: [{ ...shadows[0]!, blur: 9 }] })), first).patch.changed).toEqual(['a']);
    expect(resolveScene(parentScene(node({ id: 'a', shadows: [{ ...shadows[0]! }] })), first).patch.changed).toEqual([]);
    expect(resolveScene(parentScene(node({ id: 'a' })), first).patch.changed).toEqual(['a']);
  });

  it('adds the parent offset in parent space and keeps world space as given', () => {
    const p = resolveScene(parentScene(
      node({ id: 'f', kind: 'frame', x: 100, y: 50, w: 200, h: 100 }),
      node({ id: 'r', parent: 'f', x: 10, y: 20 }),
    )).scene;
    expect(p.byId.get('r')).toMatchObject({ x: 110, y: 70, lx: 10, ly: 20, depth: 1 });
    const w = resolveScene(worldScene(
      node({ id: 'f', kind: 'frame', x: 100, y: 50, w: 200, h: 100 }),
      node({ id: 'r', parent: 'f', x: 110, y: 70 }),
    )).scene;
    expect(w.byId.get('r')).toMatchObject({ x: 110, y: 70, lx: 10, ly: 20, depth: 1 });
  });

  it('both spaces give the same resolved scene for the same layout', () => {
    const a = resolveScene(parentScene(
      node({ id: 'f', kind: 'frame', x: 5, y: 5, w: 50, h: 50 }),
      node({ id: 'g', parent: 'f', kind: 'frame', x: 5, y: 5, w: 30, h: 30 }),
      node({ id: 'r', parent: 'g', x: 1, y: 2 }),
    )).scene;
    const b = resolveScene(worldScene(
      node({ id: 'f', kind: 'frame', x: 5, y: 5, w: 50, h: 50 }),
      node({ id: 'g', parent: 'f', kind: 'frame', x: 10, y: 10, w: 30, h: 30 }),
      node({ id: 'r', parent: 'g', x: 11, y: 12 }),
    )).scene;
    expect(b.items).toEqual(a.items);
  });

  it('orders parents before children and siblings by array order', () => {
    const rs = resolveScene(parentScene(
      node({ id: 'a', kind: 'frame' }),
      node({ id: 'b', kind: 'frame' }),
      node({ id: 'a2', parent: 'a' }),
      node({ id: 'b1', parent: 'b' }),
      node({ id: 'a1', parent: 'a' }),
    )).scene;
    expect(rs.items.map((n) => n.id)).toEqual(['a', 'a2', 'a1', 'b', 'b1']);
  });

  it('drops orphans, cycles and repeated ids', () => {
    const rs = resolveScene(parentScene(
      node({ id: 'ok' }),
      node({ id: 'orphan', parent: 'missing' }),
      node({ id: 'c1', parent: 'c2' }),
      node({ id: 'c2', parent: 'c1' }),
      node({ id: 'ok', x: 99 }),
    )).scene;
    expect(rs.items.map((n) => n.id)).toEqual(['ok']);
    expect(rs.byId.get('ok')?.x).toBe(0);
  });

  it('hides a subtree through an invisible ancestor and computes clip rectangles', () => {
    const rs = resolveScene(parentScene(
      node({ id: 'f', kind: 'frame', x: 0, y: 0, w: 100, h: 100, clip: true, visible: false }),
      node({ id: 'g', parent: 'f', kind: 'frame', x: 50, y: 50, w: 100, h: 100, clip: true }),
      node({ id: 'r', parent: 'g', x: 0, y: 0 }),
    )).scene;
    expect(rs.byId.get('r')?.visible).toBe(false);
    expect(rs.byId.get('f')?.clipRect).toBeNull();
    expect(rs.byId.get('g')?.clipRect).toEqual({ x: 0, y: 0, w: 100, h: 100 });
    expect(rs.byId.get('r')?.clipRect).toEqual({ x: 50, y: 50, w: 50, h: 50 });
  });
});

describe('incremental diff', () => {
  const base = parentScene(
    node({ id: 'f', kind: 'frame', x: 0, y: 0, w: 100, h: 100 }),
    node({ id: 'a', parent: 'f', x: 1, y: 1 }),
    node({ id: 'b', parent: 'f', x: 20, y: 20 }),
  );

  it('an identical scene reports nothing and keeps every object', () => {
    const first = resolveScene(base);
    const second = resolveScene({ ...base, nodes: base.nodes.map((n) => ({ ...n })) }, first.scene);
    expect(second.patch).toEqual({ added: [], removed: [], changed: [], orderChanged: false });
    second.scene.items.forEach((n, i) => expect(n).toBe(first.scene.items[i]));
  });

  it('a change rebuilds only that node', () => {
    const first = resolveScene(base);
    const moved = parentScene(...base.nodes.map((n) => (n.id === 'b' ? { ...n, x: 30 } : n)));
    const second = resolveScene(moved, first.scene);
    expect(second.patch.changed).toEqual(['b']);
    expect(second.patch.added).toEqual([]);
    expect(second.scene.byId.get('a')).toBe(first.scene.byId.get('a'));
    expect(second.scene.byId.get('f')).toBe(first.scene.byId.get('f'));
    expect(second.scene.byId.get('b')).not.toBe(first.scene.byId.get('b'));
  });

  it('moving a parent changes its descendants (their world rects moved)', () => {
    const first = resolveScene(base);
    const moved = parentScene(...base.nodes.map((n) => (n.id === 'f' ? { ...n, x: 10 } : n)));
    const second = resolveScene(moved, first.scene);
    expect(second.patch.changed.sort()).toEqual(['a', 'b', 'f']);
  });

  it('reports additions, removals and order changes', () => {
    const first = resolveScene(base);
    const added = resolveScene(parentScene(...base.nodes, node({ id: 'c', parent: 'f' })), first.scene);
    expect(added.patch).toMatchObject({ added: ['c'], removed: [], changed: [], orderChanged: true });
    const removed = resolveScene(parentScene(base.nodes[0]!, base.nodes[1]!), first.scene);
    expect(removed.patch).toMatchObject({ added: [], removed: ['b'], changed: [], orderChanged: true });
    const swapped = resolveScene(parentScene(base.nodes[0]!, base.nodes[2]!, base.nodes[1]!), first.scene);
    expect(swapped.patch).toMatchObject({ added: [], removed: [], changed: [], orderChanged: true });
  });

  it('a field change that is not geometry is a change too', () => {
    const first = resolveScene(base);
    const second = resolveScene(parentScene(...base.nodes.map((n) => (n.id === 'a' ? { ...n, fill: 7 } : n))), first.scene);
    expect(second.patch.changed).toEqual(['a']);
  });
});

describe('hitTest', () => {
  const rs = resolveScene(parentScene(
    node({ id: 'f', kind: 'frame', x: 0, y: 0, w: 100, h: 100, clip: true }),
    node({ id: 'back', parent: 'f', x: 10, y: 10, w: 50, h: 50 }),
    node({ id: 'front', parent: 'f', x: 30, y: 30, w: 50, h: 50 }),
    node({ id: 'out', parent: 'f', x: 90, y: 90, w: 50, h: 50 }),
    node({ id: 'hidden', parent: 'f', x: 0, y: 0, w: 100, h: 100, visible: false }),
    node({ id: 'other', kind: 'frame', x: 200, y: 0, w: 50, h: 50 }),
  )).scene;

  it('returns the front-most node (later siblings and deeper nodes win)', () => {
    expect(hitTest(rs, 40, 40)).toBe('front');
    expect(hitTest(rs, 15, 15)).toBe('back');
    expect(hitTest(rs, 95, 5)).toBe('f');
    expect(hitTest(rs, 210, 10)).toBe('other');
  });

  it('misses empty canvas, hidden nodes and the clipped-away part of a child', () => {
    expect(hitTest(rs, 500, 500)).toBeNull();
    expect(hitTest(rs, 120, 120)).toBeNull(); // inside "out", outside the clipping frame
    expect(hitTest(rs, 95, 95)).toBe('out');
  });

  it('slop widens the target', () => {
    expect(hitTest(rs, 203, -2, 0)).toBeNull();
    expect(hitTest(rs, 198, -2, 3)).toBe('other');
  });
});

describe('bounds', () => {
  const rs = resolveScene(parentScene(
    node({ id: 'a', kind: 'frame', x: 0, y: 0, w: 10, h: 10 }),
    node({ id: 'b', kind: 'frame', x: 100, y: 50, w: 20, h: 20 }),
    node({ id: 'c', parent: 'b', x: 500, y: 500 }),
    node({ id: 'h', kind: 'frame', x: 9000, y: 9000, visible: false }),
  )).scene;
  it('contentBounds covers the visible nodes on the canvas', () => {
    expect(contentBounds(rs)).toEqual({ x: 0, y: 0, w: 120, h: 70 });
    expect(contentBounds(resolveScene(parentScene()).scene)).toBeNull();
  });
  it('rectsOf skips unknown ids', () => {
    expect(rectsOf(rs, ['b', 'nope'])).toEqual([{ x: 100, y: 50, w: 20, h: 20 }]);
  });
});

describe('capture box', () => {
  const border = (position: 'inside' | 'center' | 'outside', width = 6) => ({ width, color: 1, alpha: 1, position });
  const rs = resolveScene(parentScene(
    node({ id: 'in', x: 100, y: 100, w: 50, h: 40, border: border('inside') }),
    node({ id: 'mid', x: 200, y: 100, w: 50, h: 40, border: border('center') }),
    node({ id: 'out', x: 300, y: 100, w: 50, h: 40, border: border('outside') }),
    node({ id: 'plain', x: 400, y: 100, w: 50, h: 40 }),
    node({ id: 'kid', parent: 'out', x: 5, y: 5, w: 10, h: 10, border: border('outside') }),
  )).scene;

  it('a node is captured with the band of its border that lies outside it, and the rectangle used to select it is not grown', () => {
    expect(captureRectOf(rs, 'in')).toEqual({ x: 100, y: 100, w: 50, h: 40 });
    expect(captureRectOf(rs, 'mid')).toEqual({ x: 197, y: 97, w: 56, h: 46 });
    expect(captureRectOf(rs, 'out')).toEqual({ x: 294, y: 94, w: 62, h: 52 });
    expect(captureRectOf(rs, 'plain')).toEqual({ x: 400, y: 100, w: 50, h: 40 });
    expect(captureRectOf(rs, 'nope')).toBeNull();
    expect(rectsOf(rs, ['out'])[0]).toEqual({ x: 300, y: 100, w: 50, h: 40 });
  });

  it('the whole page is captured with the band of the borders of the nodes on the canvas', () => {
    expect(captureBounds(rs)).toEqual({ x: 100, y: 94, w: 350, h: 52 });
    // The bounds of the content itself (what fit shows) are unchanged.
    expect(contentBounds(rs)).toEqual({ x: 100, y: 100, w: 350, h: 40 });
    expect(captureBounds(resolveScene(parentScene()).scene)).toBeNull();
  });

  it('a hidden node adds nothing to the page', () => {
    const hidden = resolveScene(parentScene(node({ id: 'a', w: 10, h: 10 }), node({ id: 'b', x: 500, visible: false, border: border('outside', 20) }))).scene;
    expect(captureBounds(hidden)).toEqual({ x: 0, y: 0, w: 10, h: 10 });
  });
});

describe('capture box with shadows', () => {
  const sh = (o = {}) => ({ x: 0, y: 0, blur: 0, spread: 0, color: 0, alpha: 1, ...o });
  const rs = resolveScene(parentScene(
    node({ id: 'a', x: 100, y: 100, w: 50, h: 40, shadows: [sh({ y: 4, blur: 10 })] }),
    node({ id: 'b', x: 300, y: 100, w: 50, h: 40, border: { width: 6, color: 1, alpha: 1, position: 'outside' }, shadows: [sh({ x: 20, spread: 2 })] }),
    node({ id: 'c', x: 500, y: 100, w: 50, h: 40, shadows: [sh({ spread: -30, blur: 20 })] }),
    node({ id: 'frame', kind: 'frame', x: 600, y: 100, w: 100, h: 100, clip: true }),
    node({ id: 'in', parent: 'frame', x: 0, y: 0, w: 20, h: 20, shadows: [sh({ blur: 50 })] }),
    node({ id: 'free', kind: 'frame', x: 800, y: 100, w: 100, h: 100 }),
    node({ id: 'out', parent: 'free', x: 90, y: 0, w: 20, h: 20, shadows: [sh({ x: 30 })] }),
  )).scene;

  it('a node is captured with the reach of its shadows (offset, spread and blur), joined to the band of its border', () => {
    expect(captureRectOf(rs, 'a')).toEqual({ x: 90, y: 94, w: 70, h: 60 });
    expect(captureRectOf(rs, 'b')).toEqual({ x: 294, y: 94, w: 78, h: 52 });
    // A shadow with nothing left of it adds nothing, and the rectangle that selects the node is never grown.
    expect(captureRectOf(rs, 'c')).toEqual({ x: 500, y: 100, w: 50, h: 40 });
    expect(rectsOf(rs, ['a'])[0]).toEqual({ x: 100, y: 100, w: 50, h: 40 });
  });

  it('the page is captured with the shadows of nodes at any depth, cut by a clipping ancestor and not by one that does not clip', () => {
    const b = captureBounds(rs)!;
    expect(b.x).toBe(90);
    // `out` sits at the right edge of `free` (x 890) and casts 30 to the right: 890 + 20 + 30 = 940, past `free` at 900.
    expect(b.x + b.w).toBe(940);
    // The blur of `in` reaches 50 past the clipping `frame`, which cuts it: nothing above or left of the frame is in.
    expect(b.y).toBe(94);
  });
});

describe('gradient', () => {
  const g = { kind: 'linear' as const, angle: 90, stops: [{ pos: 0, color: '#ffffff' }, { pos: 1, color: '#000000' }] };
  const node = (extra = {}) => ({ id: 'a', parent: null, kind: 'rect' as const, x: 0, y: 0, w: 10, h: 10, fill: 0x112233, ...extra });
  it('is carried to the resolved node, and absent when the node has none', () => {
    expect(resolveScene({ space: 'parent', nodes: [node({ gradient: g })] }).scene.items[0]!.gradient).toEqual(g);
    expect(resolveScene({ space: 'parent', nodes: [node()] }).scene.items[0]!.gradient).toBeUndefined();
  });
  it('a changed gradient makes the node changed, an equal one does not', () => {
    const first = resolveScene({ space: 'parent', nodes: [node({ gradient: g })] }).scene;
    const same = resolveScene({ space: 'parent', nodes: [node({ gradient: { ...g, stops: g.stops.map((s) => ({ ...s })) } })] }, first);
    expect(same.patch.changed).toEqual([]);
    const other = resolveScene({ space: 'parent', nodes: [node({ gradient: { ...g, angle: 180 } })] }, first);
    expect(other.patch.changed).toEqual(['a']);
    const gone = resolveScene({ space: 'parent', nodes: [node()] }, first);
    expect(gone.patch.changed).toEqual(['a']);
  });
});

describe('image', () => {
  const img = { assetId: 'a'.repeat(64), fit: 'cover' as const };
  const node = (extra = {}) => ({ id: 'a', parent: null, kind: 'rect' as const, x: 0, y: 0, w: 10, h: 10, fill: 0x112233, ...extra });
  it('is carried to the resolved node, and absent when the node has none', () => {
    expect(resolveScene({ space: 'parent', nodes: [node({ image: img })] }).scene.items[0]!.image).toEqual(img);
    expect(resolveScene({ space: 'parent', nodes: [node()] }).scene.items[0]!.image).toBeUndefined();
  });
  it('a changed image or fit makes the node changed, an equal one does not', () => {
    const first = resolveScene({ space: 'parent', nodes: [node({ image: img })] }).scene;
    const same = resolveScene({ space: 'parent', nodes: [node({ image: { ...img } })] }, first);
    expect(same.patch.changed).toEqual([]);
    const fit = resolveScene({ space: 'parent', nodes: [node({ image: { ...img, fit: 'contain' } })] }, first);
    expect(fit.patch.changed).toEqual(['a']);
    const other = resolveScene({ space: 'parent', nodes: [node({ image: { ...img, assetId: 'b'.repeat(64) } })] }, first);
    expect(other.patch.changed).toEqual(['a']);
    const gone = resolveScene({ space: 'parent', nodes: [node()] }, first);
    expect(gone.patch.changed).toEqual(['a']);
  });
});
