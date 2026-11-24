import { describe, expect, it } from 'vitest';
import { Container, Graphics, type RoundedRectangle } from 'pixi.js';
import type { SceneBorder } from '@meldframe/web-canvas/border';
import { resolveScene, type Scene, type SceneNode } from '@meldframe/web-canvas/scene';
import { WorldView } from './stage';

// The border of a node (issue 236): a ring drawn over the node and its children, which the node's own clip does not cut.
const border = (o: Partial<SceneBorder> = {}): SceneBorder => ({ width: 6, color: 0xff0000, alpha: 1, position: 'inside', ...o });
const node = (o: Partial<SceneNode> & Pick<SceneNode, 'id'>): SceneNode => ({
  parent: null, kind: 'frame', x: 0, y: 0, w: 100, h: 60, fill: 0x808080, clip: false, ...o,
});
const scene = (...nodes: SceneNode[]): Scene => ({ space: 'parent', nodes });

const draw = (...nodes: SceneNode[]) => {
  const world = new WorldView();
  const r = resolveScene(scene(...nodes));
  world.apply(r.scene, r.patch);
  return { world, r };
};
const top = (world: WorldView, i = 0) => world.root.children[i] as Container;
const last = (c: Container) => c.children[c.children.length - 1] as Graphics;

/** The ring a border Graphics draws: the filled shape, its style, and the hole cut out of it. */
function ring(g: Graphics) {
  const fills = g.context.instructions.filter((i) => i.action === 'fill');
  expect(fills).toHaveLength(1);
  const data = fills[0]!.data as unknown as { style: { color: number; alpha: number }; path: { shapePath: { shapePrimitives: { shape: RoundedRectangle }[] } }; hole?: { shapePath: { shapePrimitives: { shape: RoundedRectangle }[] } } };
  const shape = (s: RoundedRectangle | undefined) => (s ? { x: s.x, y: s.y, w: s.width, h: s.height, r: s.radius } : null);
  return {
    style: data.style,
    outer: shape(data.path.shapePath.shapePrimitives[0]?.shape),
    inner: shape(data.hole?.shapePath.shapePrimitives[0]?.shape),
  };
}

describe('WorldView draws the border of a node', () => {
  it('a node with no border has no ring, and its body is still the first part', () => {
    const { world } = draw(node({ id: 'a' }));
    expect(top(world).children).toHaveLength(1);
  });

  it('the ring is the last part of the node, over its children, with the colour and the alpha of the border', () => {
    const { world } = draw(node({ id: 'f', border: border({ alpha: 0.5 }) }), node({ id: 'k', parent: 'f', kind: 'rect', w: 10, h: 10 }));
    const f = top(world);
    const g = last(f);
    expect(g).toBeInstanceOf(Graphics);
    // [body, content (the children), border]
    expect(f.children).toHaveLength(3);
    expect((f.children[1] as Container).children.map((c) => c.label)).toEqual(['k']);
    expect(ring(g).style).toMatchObject({ color: 0xff0000, alpha: 0.5 });
  });

  it('draws the ring of each position as the export does', () => {
    const at = (position: SceneBorder['position'], radius = 0) => ring(last(top(draw(node({ id: 'a', radius, border: border({ position }) })).world)));
    expect(at('inside')).toMatchObject({ outer: { x: 0, y: 0, w: 100, h: 60, r: 0 }, inner: { x: 6, y: 6, w: 88, h: 48, r: 0 } });
    expect(at('center')).toMatchObject({ outer: { x: -3, y: -3, w: 106, h: 66, r: 0 }, inner: { x: 3, y: 3, w: 94, h: 54, r: 0 } });
    expect(at('outside')).toMatchObject({ outer: { x: -6, y: -6, w: 112, h: 72, r: 0 }, inner: { x: 0, y: 0, w: 100, h: 60, r: 0 } });
    expect(at('outside', 10)).toMatchObject({ outer: { r: 16 }, inner: { r: 10 } });
  });

  it('an ellipse is a pill: its border follows the radius the render step gave it', () => {
    const { world } = draw(node({ id: 'e', kind: 'rect', w: 50, h: 50, radius: 25, border: border({ position: 'center' }) }));
    expect(ring(last(top(world)))).toMatchObject({ outer: { w: 56, h: 56, r: 28 }, inner: { w: 44, h: 44, r: 22 } });
  });

  it('a box with no height or no width gets its ring as the export draws it: the border alone, with no hole', () => {
    // A divider: 200 x 0 with a 2 px border inside. The export's ::after has a content height of 0 and its two borders: 4 px from the top.
    const g = last(top(draw(node({ id: 'd', w: 200, h: 0, border: border({ width: 2 }) })).world));
    expect(g).toBeInstanceOf(Graphics);
    expect(ring(g)).toMatchObject({ outer: { x: 0, y: 0, w: 200, h: 4 }, inner: null });
    const outside = last(top(draw(node({ id: 'e', w: 0, h: 0, border: border({ width: 2, position: 'outside' }) })).world));
    expect(ring(outside)).toMatchObject({ outer: { x: -2, y: -2, w: 4, h: 4 }, inner: null });
  });

  it('a size that is not a number draws no ring', () => {
    for (const o of [{ w: NaN }, { h: NaN }, { w: Infinity }]) {
      const c = top(draw(node({ id: 'n', ...o, border: border() })).world);
      expect(c.children.every((k) => !(k instanceof Graphics)), JSON.stringify(o)).toBe(true);
    }
  });

  it('a border as wide as half the box paints the whole box: no hole', () => {
    const { world } = draw(node({ id: 'a', border: border({ width: 40 }) }));
    expect(ring(last(top(world))).inner).toBeNull();
  });

  it('the clip of a frame cuts its children and not its own border', () => {
    const { world } = draw(node({ id: 'f', clip: true, border: border({ position: 'outside' }) }), node({ id: 'k', parent: 'f', kind: 'rect', w: 200, h: 200 }));
    const f = top(world);
    const content = f.children[1] as Container;
    expect(content.mask).toBeTruthy();
    expect(f.mask).toBeFalsy();
    expect(last(f).parent).toBe(f);
    expect(last(f)).not.toBe(content);
    expect(content.children.map((c) => c.label)).toContain('k');
  });

  it('a change of the border, of the box or of the radius draws the ring again; nothing else does', () => {
    const world = new WorldView();
    let prev = resolveScene(scene(node({ id: 'a', border: border() })));
    world.apply(prev.scene, prev.patch);
    const g = last(top(world));
    const next = (o: Partial<SceneNode>) => {
      prev = resolveScene(scene(node({ id: 'a', border: border(), ...o })), prev.scene);
      world.apply(prev.scene, prev.patch);
      return last(top(world));
    };
    expect(next({ border: border({ width: 10 }) })).toBe(g);
    expect(ring(g).inner).toMatchObject({ x: 10, w: 80 });
    next({ w: 200 });
    expect(ring(g).outer).toMatchObject({ w: 200 });
    next({ w: 200, radius: 12 });
    expect(ring(g).outer).toMatchObject({ r: 12 });
    next({ w: 200, radius: 12, border: border({ color: 0x00ff00 }) });
    expect(ring(g).style).toMatchObject({ color: 0x00ff00 });
  });

  it('removing the border removes the ring, and a border can come back', () => {
    const world = new WorldView();
    let prev = resolveScene(scene(node({ id: 'a', border: border() })));
    world.apply(prev.scene, prev.patch);
    prev = resolveScene(scene(node({ id: 'a' })), prev.scene);
    world.apply(prev.scene, prev.patch);
    expect(top(world).children).toHaveLength(1);
    prev = resolveScene(scene(node({ id: 'a', border: border() })), prev.scene);
    world.apply(prev.scene, prev.patch);
    expect(last(top(world))).toBeInstanceOf(Graphics);
    expect(top(world).children).toHaveLength(2);
  });

  it('a child that moves to another parent keeps its own border and its children', () => {
    const world = new WorldView();
    const a = [node({ id: 'p', clip: true }), node({ id: 'q' , x: 300}), node({ id: 'k', parent: 'p', kind: 'rect', border: border() })];
    let prev = resolveScene(scene(...a));
    world.apply(prev.scene, prev.patch);
    prev = resolveScene(scene(a[0]!, a[1]!, { ...a[2]!, parent: 'q' }), prev.scene);
    expect(() => world.apply(prev.scene, prev.patch)).not.toThrow();
    const q = top(world, 1);
    const content = q.children.find((c) => (c as Container).children.some((k) => k.label === 'k')) as Container;
    const k = content.children.find((c) => c.label === 'k') as Container;
    expect(k.destroyed).toBe(false);
    expect(last(k)).toBeInstanceOf(Graphics);
  });
});
