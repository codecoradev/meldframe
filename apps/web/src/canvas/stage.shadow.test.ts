import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { BlurFilter, Container, Graphics, type RoundedRectangle } from 'pixi.js';
import { MAX_BLUR, type SceneShadow } from '@meldframe/web-canvas/shadow';
import { resolveScene, type Scene, type SceneNode } from '@meldframe/web-canvas/scene';
import { WorldView } from './stage';

// The shadows of a node (issue 236): behind the node, in model order, outside the node's own clip, blurred by a filter whose
// strength follows the zoom (a filter works in screen pixels, under the camera's scale).
const shadow = (o: Partial<SceneShadow> = {}): SceneShadow => ({ x: 0, y: 4, blur: 0, spread: 0, color: 0x000000, alpha: 0.5, ...o });
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

/** What a shadow Graphics draws: its one filled rounded rectangle and the style of the fill. */
function drawn(g: Graphics) {
  const fills = g.context.instructions.filter((i) => i.action === 'fill');
  expect(fills).toHaveLength(1);
  const data = fills[0]!.data as unknown as { style: { color: number; alpha: number }; path: { shapePath: { shapePrimitives: { shape: RoundedRectangle }[] } } };
  const s = data.path.shapePath.shapePrimitives[0]!.shape;
  return { style: data.style, x: s.x, y: s.y, w: s.width, h: s.height, r: s.radius };
}
const blurOf = (g: Graphics) => (g.filters as BlurFilter[] | null)?.[0];
/** The shadow parts of a node container: everything before its body, which is the one Sprite. */
const shadowsOf = (c: Container) => c.children.filter((k) => k.label === 'shadow') as Graphics[];

describe('WorldView draws the shadows of a node', () => {
  // A blur filter compiles its program against a test WebGL context, which a test in Node does not have: with none, Pixi falls back to a precision.
  beforeEach(() => vi.stubGlobal('document', { createElement: () => ({ getContext: () => null }) }));
  afterEach(() => vi.unstubAllGlobals());

  it('a node with no shadow has no shadow part', () => {
    const { world } = draw(node({ id: 'a' }), node({ id: 'b', shadows: [] }));
    expect(shadowsOf(top(world, 0))).toHaveLength(0);
    expect(shadowsOf(top(world, 1))).toHaveLength(0);
  });

  it('the shadows come first in the node, before its body, and in model order (the first is the bottom)', () => {
    const { world } = draw(node({ id: 'a', shadows: [shadow({ color: 0xff0000 }), shadow({ color: 0x0000ff, alpha: 1 })] }));
    const c = top(world);
    const [s0, s1] = shadowsOf(c);
    expect(c.children.indexOf(s0!)).toBe(0);
    expect(c.children.indexOf(s1!)).toBe(1);
    // The body (a sprite) is drawn over both.
    expect(c.children[2]).not.toBeInstanceOf(Graphics);
    expect(drawn(s0!).style).toMatchObject({ color: 0xff0000, alpha: 0.5 });
    expect(drawn(s1!).style).toMatchObject({ color: 0x0000ff, alpha: 1 });
  });

  it('a shadow has the shape of the box moved by its offset and grown by its spread, with the radius that the spread leaves', () => {
    const at = (s: Partial<SceneShadow>, radius = 0) => drawn(shadowsOf(top(draw(node({ id: 'a', radius, shadows: [shadow(s)] })).world))[0]!);
    expect(at({ x: 5, y: -3 })).toMatchObject({ x: 5, y: -3, w: 100, h: 60, r: 0 });
    expect(at({ x: 0, y: 0, spread: 8 }, 10)).toMatchObject({ x: -8, y: -8, w: 116, h: 76, r: 18 });
    expect(at({ x: 0, y: 0, spread: -4 }, 10)).toMatchObject({ x: 4, y: 4, w: 92, h: 52, r: 6 });
  });

  it('an ellipse is a pill: its shadow follows the radius the render step gave it', () => {
    const { world } = draw(node({ id: 'e', kind: 'rect', w: 50, h: 50, radius: 25, shadows: [shadow({ y: 0, spread: 10 })] }));
    expect(drawn(shadowsOf(top(world))[0]!)).toMatchObject({ w: 70, h: 70, r: 35 });
  });

  it('a shadow with nothing left of it, or on a size that is not a number, draws nothing', () => {
    expect(shadowsOf(top(draw(node({ id: 'a', shadows: [shadow({ spread: -40 })] })).world))).toHaveLength(0);
    for (const o of [{ w: NaN }, { h: NaN }, { w: Infinity }]) {
      expect(shadowsOf(top(draw(node({ id: 'n', ...o, shadows: [shadow()] })).world)), JSON.stringify(o)).toHaveLength(0);
    }
  });

  it('a hard shadow has no filter and a blurred one has a blur whose strength is half its radius, in screen pixels', () => {
    const { world } = draw(node({ id: 'a', shadows: [shadow(), shadow({ blur: 12 })] }));
    const [hard, soft] = shadowsOf(top(world));
    expect(blurOf(hard!)).toBeUndefined();
    expect(blurOf(soft!)).toBeInstanceOf(BlurFilter);
    expect(blurOf(soft!)!.strength).toBe(6);
    expect(blurOf(soft!)!.quality).toBe(4);
    // Room for the tail of the blur: the padding is 1.5 times the radius and a little more.
    expect(blurOf(soft!)!.padding).toBe(Math.ceil(1.5 * 12) + 2);
  });

  it('the strength of the blur follows the zoom of the camera: the filter works in screen pixels and the world is scaled', () => {
    const { world } = draw(node({ id: 'a', shadows: [shadow({ blur: 12 })] }), node({ id: 'b', x: 200, shadows: [shadow({ blur: 4 }), shadow()] }));
    const strengths = () => [blurOf(shadowsOf(top(world, 0))[0]!)!.strength, blurOf(shadowsOf(top(world, 1))[0]!)!.strength];
    world.setCamera({ x: 0, y: 0, zoom: 2 });
    expect(strengths()).toEqual([12, 4]);
    expect(blurOf(shadowsOf(top(world, 0))[0]!)!.padding).toBe(Math.ceil(1.5 * 12 * 2) + 2);
    world.setCamera({ x: 30, y: 10, zoom: 0.25 });
    expect(strengths()).toEqual([1.5, 0.5]);
    world.setCamera({ x: 0, y: 0, zoom: 1 });
    expect(strengths()).toEqual([6, 2]);
  });

  it('a shadow that is drawn after the camera has moved starts at the strength of the zoom', () => {
    const world = new WorldView();
    world.setCamera({ x: 0, y: 0, zoom: 3 });
    const r = resolveScene(scene(node({ id: 'a', shadows: [shadow({ blur: 10 })] })));
    world.apply(r.scene, r.patch);
    expect(blurOf(shadowsOf(top(world))[0]!)!.strength).toBe(15);
  });

  it('the shadows are not in the content of the node, so its clip does not cut them; the clip still cuts its children', () => {
    const { world } = draw(node({ id: 'f', clip: true, shadows: [shadow({ blur: 8 })] }), node({ id: 'k', parent: 'f', kind: 'rect', w: 200, h: 200 }));
    const f = top(world);
    const content = f.children.find((k) => k.label === 'content') as Container;
    expect(content.mask).toBeTruthy();
    expect(f.mask).toBeFalsy();
    expect(shadowsOf(f)).toHaveLength(1);
    expect(shadowsOf(content)).toHaveLength(0);
    // [shadow, body, content]
    expect(f.children.indexOf(shadowsOf(f)[0]!)).toBeLessThan(f.children.indexOf(content));
    expect(content.children.map((c) => c.label)).toContain('k');
  });

  it('the order of a node is its shadows, body, text, children and border', () => {
    const { world } = draw(
      node({ id: 'f', shadows: [shadow()], border: { width: 4, color: 1, alpha: 1, position: 'inside' } }),
      node({ id: 'k', parent: 'f', kind: 'rect', w: 10, h: 10 }),
    );
    const f = top(world);
    expect(f.children.map((c) => c.label)).toEqual(['shadow', 'Sprite', 'content', 'Graphics']);
    expect(f.children[3]).toBeInstanceOf(Graphics);
  });

  it('a change of the shadows, of the box or of the radius draws them again, and nothing else does', () => {
    const world = new WorldView();
    let prev = resolveScene(scene(node({ id: 'a', shadows: [shadow()] })));
    world.apply(prev.scene, prev.patch);
    const g = shadowsOf(top(world))[0]!;
    const next = (o: Partial<SceneNode>) => {
      prev = resolveScene(scene(node({ id: 'a', shadows: [shadow()], ...o })), prev.scene);
      world.apply(prev.scene, prev.patch);
      return shadowsOf(top(world))[0]!;
    };
    expect(next({ shadows: [shadow({ color: 0x00ff00 })] })).toBe(g);
    expect(drawn(g).style).toMatchObject({ color: 0x00ff00 });
    next({ w: 200, shadows: [shadow()] });
    expect(drawn(g)).toMatchObject({ w: 200 });
    next({ w: 200, radius: 12, shadows: [shadow()] });
    expect(drawn(g)).toMatchObject({ r: 12 });
    // A blur that arrives gives the part a filter, and one that goes takes it away.
    next({ w: 200, radius: 12, shadows: [shadow({ blur: 6 })] });
    expect(blurOf(g)?.strength).toBe(3);
    next({ w: 200, radius: 12, shadows: [shadow()] });
    expect(blurOf(g)).toBeUndefined();
  });

  it('the number of shadow parts follows the list, and removing the shadows removes them all', () => {
    const world = new WorldView();
    let prev = resolveScene(scene(node({ id: 'a', shadows: [shadow(), shadow({ blur: 2 }), shadow({ x: 3 })] })));
    world.apply(prev.scene, prev.patch);
    expect(shadowsOf(top(world))).toHaveLength(3);
    const dropped = shadowsOf(top(world))[2]!;
    prev = resolveScene(scene(node({ id: 'a', shadows: [shadow()] })), prev.scene);
    world.apply(prev.scene, prev.patch);
    expect(shadowsOf(top(world))).toHaveLength(1);
    expect(dropped.destroyed).toBe(true);
    prev = resolveScene(scene(node({ id: 'a' })), prev.scene);
    world.apply(prev.scene, prev.patch);
    expect(top(world).children.filter((c) => c instanceof Graphics)).toHaveLength(0);
    prev = resolveScene(scene(node({ id: 'a', shadows: [shadow({ blur: 4 })] })), prev.scene);
    world.apply(prev.scene, prev.patch);
    expect(shadowsOf(top(world))).toHaveLength(1);
  });

  it('a node that is removed takes its shadows and their filters with it', () => {
    const world = new WorldView();
    let prev = resolveScene(scene(node({ id: 'a', shadows: [shadow({ blur: 4 })] })));
    world.apply(prev.scene, prev.patch);
    const g = shadowsOf(top(world))[0]!;
    prev = resolveScene(scene(), prev.scene);
    world.apply(prev.scene, prev.patch);
    expect(g.destroyed).toBe(true);
    world.setCamera({ x: 0, y: 0, zoom: 2 });
  });

  it('a shadow with an alpha of 0 draws nothing and has no filter', () => {
    const { world } = draw(node({ id: 'a', shadows: [shadow({ alpha: 0, blur: 8 }), shadow({ color: 0x00ff00 })] }));
    const parts = shadowsOf(top(world));
    expect(parts).toHaveLength(1);
    expect(drawn(parts[0]!).style).toMatchObject({ color: 0x00ff00 });
    expect(blurOf(parts[0]!)).toBeUndefined();
  });

  it('the bounds of a blurred shadow include its halo, which the culling reads (it ignores the padding of a filter); a hard one has its own', () => {
    const { world } = draw(node({ id: 'a', shadows: [shadow({ blur: 12 }), shadow({ y: 0 })] }));
    const [soft, hard] = shadowsOf(top(world));
    // The padding of the filter in world units: 1.5 times the blur and 2, around the shape (0, 4, 100, 60). It is said beside the
    // shadow, not on it: a boundsArea on the Graphics of a filter breaks the filter.
    expect(soft!.boundsArea).toBeFalsy();
    const halos = top(world).children.filter((c) => c.label === 'shadow-halo');
    expect(halos).toHaveLength(1);
    expect(halos[0]!.boundsArea).toMatchObject({ x: -20, y: 4 - 20, width: 140, height: 100 });
    expect(hard!.boundsArea).toBeFalsy();
    // So the node's own bounds, which the culling reads, include it.
    const b = top(world).getLocalBounds();
    expect([b.minX, b.minY, b.maxX, b.maxY]).toEqual([-20, -16, 120, 84]);
  });

  it('a halo with a huge blur stops at the cap, and a blur that goes takes its halo away', () => {
    const { world } = draw(node({ id: 'a', shadows: [shadow({ blur: 100000 })] }));
    const pad = Math.ceil(1.5 * MAX_BLUR) + 2;
    const halo = (w: WorldView) => top(w).children.filter((c) => c.label === 'shadow-halo');
    expect(halo(world)[0]!.boundsArea).toMatchObject({ x: -pad, width: 100 + 2 * pad });
    const w2 = new WorldView();
    let prev = resolveScene(scene(node({ id: 'a', shadows: [shadow({ blur: 6 })] })));
    w2.apply(prev.scene, prev.patch);
    expect(halo(w2)).toHaveLength(1);
    prev = resolveScene(scene(node({ id: 'a', shadows: [shadow()] })), prev.scene);
    w2.apply(prev.scene, prev.patch);
    expect(halo(w2)).toHaveLength(0);
    // And when the shadows go altogether.
    prev = resolveScene(scene(node({ id: 'a', shadows: [shadow({ blur: 6 })] })), prev.scene);
    w2.apply(prev.scene, prev.patch);
    expect(halo(w2)).toHaveLength(1);
    prev = resolveScene(scene(node({ id: 'a' })), prev.scene);
    w2.apply(prev.scene, prev.patch);
    expect(halo(w2)).toHaveLength(0);
  });

  it('the strength and the padding of a blur are capped: a blur of 5000 at a zoom of 2 is not a padding of 15000', () => {
    const { world } = draw(node({ id: 'a', shadows: [shadow({ blur: 5000 })] }));
    world.setCamera({ x: 0, y: 0, zoom: 2 });
    const f = blurOf(shadowsOf(top(world))[0]!)!;
    expect(f.strength).toBe(MAX_BLUR / 2);
    expect(f.padding).toBe(Math.ceil(1.5 * MAX_BLUR) + 2);
  });

  it('only the nodes with a blurred shadow are walked when the zoom changes', () => {
    const { world } = draw(node({ id: 'a', shadows: [shadow({ blur: 6 })] }), node({ id: 'b', shadows: [shadow()] }), node({ id: 'c' }));
    expect((world as unknown as { blurred: Set<unknown> }).blurred.size).toBe(1);
    const w2 = new WorldView();
    let prev = resolveScene(scene(node({ id: 'a', shadows: [shadow({ blur: 6 })] })));
    w2.apply(prev.scene, prev.patch);
    const set = () => (w2 as unknown as { blurred: Set<unknown> }).blurred.size;
    expect(set()).toBe(1);
    prev = resolveScene(scene(node({ id: 'a', shadows: [shadow()] })), prev.scene);
    w2.apply(prev.scene, prev.patch);
    expect(set()).toBe(0);
    prev = resolveScene(scene(node({ id: 'a', shadows: [shadow({ blur: 6 })] })), prev.scene);
    w2.apply(prev.scene, prev.patch);
    expect(set()).toBe(1);
    prev = resolveScene(scene(), prev.scene);
    w2.apply(prev.scene, prev.patch);
    expect(set()).toBe(0);
  });
});
