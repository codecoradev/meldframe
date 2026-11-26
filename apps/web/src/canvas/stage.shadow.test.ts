import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { BlurFilter, Container, Graphics, type RoundedRectangle } from 'pixi.js';
import { MAX_BLUR, type SceneShadow } from '@meldframe/web-paint/shadow';
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
/** The container of a node's shadows (the parts and the mask that cuts the box out of them), none when the node has no shadow. */
const groupOf = (c: Container) => c.children.find((k) => k.label === 'shadows') as Container | undefined;
/** The shadow parts of a node container: the Graphics of that container, in order. */
const shadowsOf = (c: Container) => (groupOf(c)?.children.filter((k) => k.label === 'shadow') ?? []) as Graphics[];
/** The halo containers of a node's blurred shadows (what the culling reads). */
const halosOf = (c: Container) => groupOf(c)?.children.filter((k) => k.label === 'shadow-halo') ?? [];

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
    const group = groupOf(c)!;
    expect(group.children.indexOf(s0!)).toBe(0);
    expect(group.children.indexOf(s1!)).toBe(1);
    // The container of the shadows is the first part of the node, and the body (a sprite) is drawn over it.
    expect(c.children.indexOf(group)).toBe(0);
    expect(c.children[1]).not.toBeInstanceOf(Graphics);
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
    // [shadows, body, content]
    expect(f.children.indexOf(groupOf(f)!)).toBeLessThan(f.children.indexOf(content));
    expect(content.children.map((c) => c.label)).toContain('k');
  });

  it('the order of a node is its shadows, body, text, children and border', () => {
    const { world } = draw(
      node({ id: 'f', shadows: [shadow()], border: { width: 4, color: 1, alpha: 1, position: 'inside' } }),
      node({ id: 'k', parent: 'f', kind: 'rect', w: 10, h: 10 }),
    );
    const f = top(world);
    expect(f.children.map((c) => c.label)).toEqual(['shadows', 'Sprite', 'content', 'Graphics']);
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
    const halos = halosOf(top(world));
    expect(halos).toHaveLength(1);
    expect(halos[0]!.boundsArea).toMatchObject({ x: -20, y: 4 - 20, width: 140, height: 100 });
    expect(hard!.boundsArea).toBeFalsy();
    // So the node's own bounds, which the culling reads, include it.
    const b = top(world).getLocalBounds();
    expect([b.minX, b.minY, b.maxX, b.maxY]).toEqual([-20, -16, 120, 84]);
  });

  it('the halo of a huge blur is as wide as the uncapped blur (it is only a culling hint), and a blur that goes takes its halo away', () => {
    const { world } = draw(node({ id: 'a', shadows: [shadow({ blur: 100000 })] }));
    const pad = Math.ceil(1.5 * 100000) + 2;
    const halo = (w: WorldView) => halosOf(top(w));
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

  // The box is cut out of the shadows (issue 248): CSS draws no box-shadow inside the border box.
  /** The shape of the mask of a node's container of shadows: the rectangle it fills and the hole cut out of it. */
  const maskOf = (c: Container) => {
    const g = groupOf(c)!.mask as Graphics;
    const fills = g.context.instructions.filter((i) => i.action === 'fill');
    expect(fills).toHaveLength(1);
    const d = fills[0]!.data as unknown as { path: { shapePath: { shapePrimitives: { shape: RoundedRectangle }[] } }; hole?: { shapePath: { shapePrimitives: { shape: RoundedRectangle }[] } } };
    const shape = (p: { shape: RoundedRectangle }) => ({ x: p.shape.x, y: p.shape.y, w: p.shape.width, h: p.shape.height, r: p.shape.radius });
    return { outer: d.path.shapePath.shapePrimitives.map(shape), hole: d.hole?.shapePath.shapePrimitives.map(shape) };
  };

  it('the container of the shadows has a mask that is a rectangle holding every shadow and its halo, with the box cut out of it (the box, not the grown shape)', () => {
    const { world } = draw(node({ id: 'a', radius: 10, shadows: [shadow({ x: 5, y: 5, blur: 8, spread: 2 }), shadow({ x: -30, y: 0, spread: 4 })] }));
    const c = top(world);
    const group = groupOf(c)!;
    // The mask is a child of the group, as a clip mask is of its content; it is not one of the parts.
    expect(group.mask).toBeTruthy();
    expect(group.children).toContain(group.mask);
    expect(shadowsOf(c)).toHaveLength(2);
    const { outer, hole } = maskOf(c);
    // The second shadow (hard: a margin of 2) starts at x = -30 - 4 = -34, so at -36; the first has a halo of 14 around (3, 3, 104, 64): from -11 to 121 and 81.
    expect(outer).toMatchObject([{ x: -36, y: -11, w: 157, h: 92 }]);
    // The hole is the box with its radius, not the shape of a shadow (no spread, no offset).
    expect(hole).toMatchObject([{ x: 0, y: 0, w: 100, h: 60, r: 10 }]);
  });

  it('the hole follows the box when it changes, and the container is kept (a hard shadow too has the cut)', () => {
    const world = new WorldView();
    let prev = resolveScene(scene(node({ id: 'a', shadows: [shadow()] })));
    world.apply(prev.scene, prev.patch);
    const group = groupOf(top(world))!;
    const mask = group.mask;
    expect(maskOf(top(world)).hole).toMatchObject([{ w: 100, h: 60, r: 0 }]);
    prev = resolveScene(scene(node({ id: 'a', w: 40, h: 30, radius: 99, shadows: [shadow()] })), prev.scene);
    world.apply(prev.scene, prev.patch);
    expect(groupOf(top(world))).toBe(group);
    expect(group.mask).toBe(mask);
    // The radius is held to half the smaller side, as the body does.
    expect(maskOf(top(world)).hole).toMatchObject([{ w: 40, h: 30, r: 15 }]);
  });

  it('the mask is a normal stencil mask, not an inverse one (an inverse mask lets pixels outside a clipping ancestor through)', () => {
    const { world } = draw(node({ id: 'a', shadows: [shadow({ blur: 4 })] }));
    const group = groupOf(top(world))!;
    // `effects` is public: the mask of a Graphics is a stencil effect. The inverse flag has no public reader, so it is read from the
    // container's `_maskOptions`, a private field of Pixi 8.23 (what its stencil pipe reads).
    expect((group.effects ?? []).map((e) => (e as unknown as { pipe: string }).pipe)).toContain('stencilMask');
    expect((group as unknown as { _maskOptions: { inverse: boolean } })._maskOptions.inverse).toBe(false);
  });

  it('a shadow wholly inside the box (a negative spread) still has one rectangle with one hole, and the hole is strictly inside the rectangle', () => {
    const { world } = draw(node({ id: 'n', w: 100, h: 60, shadows: [shadow({ x: 0, y: 0, spread: -10 })] }));
    expect(shadowsOf(top(world))).toHaveLength(1);
    const { outer, hole } = maskOf(top(world));
    expect(outer).toHaveLength(1);
    expect(hole).toHaveLength(1);
    expect(hole).toMatchObject([{ x: 0, y: 0, w: 100, h: 60 }]);
    // Grown by 1 unit at least on each side: the rectangle is never the hole.
    expect(outer).toMatchObject([{ x: -1, y: -1, w: 102, h: 62 }]);
  });

  it('a box whose size is not a number has no container of shadows; a box with no width keeps its shadow and has no mask', () => {
    const w = new WorldView();
    const r = resolveScene(scene(node({ id: 'n', w: NaN, shadows: [shadow()] }), node({ id: 'z', w: 0, h: 40, shadows: [shadow({ spread: 6 })] })));
    w.apply(r.scene, r.patch);
    expect(groupOf(top(w, 0))).toBeUndefined();
    expect(shadowsOf(top(w, 1))).toHaveLength(1);
    expect(groupOf(top(w, 1))!.mask).toBeFalsy();
  });

  it('a box with no area has nothing to cut out: the container of its shadows has no mask', () => {
    const { world } = draw(node({ id: 'z', w: 0, h: 40, shadows: [shadow({ spread: 6 })] }));
    expect(shadowsOf(top(world))).toHaveLength(1);
    expect(groupOf(top(world))!.mask).toBeFalsy();
  });

  it('a node with a shadow has a container for them; one without has none, and the container, its mask and its parts go with the shadows and with the node', () => {
    const world = new WorldView();
    let prev = resolveScene(scene(node({ id: 'a' }), node({ id: 'b', shadows: [shadow({ blur: 5 })] })));
    world.apply(prev.scene, prev.patch);
    expect(groupOf(top(world, 0))).toBeUndefined();
    const group = groupOf(top(world, 1))!;
    const mask = group.mask as Graphics;
    const part = shadowsOf(top(world, 1))[0]!;
    prev = resolveScene(scene(node({ id: 'a' }), node({ id: 'b' })), prev.scene);
    world.apply(prev.scene, prev.patch);
    expect(groupOf(top(world, 1))).toBeUndefined();
    expect([group.destroyed, mask.destroyed, part.destroyed]).toEqual([true, true, true]);
    // And with the node.
    prev = resolveScene(scene(node({ id: 'a' }), node({ id: 'b', shadows: [shadow()] })), prev.scene);
    world.apply(prev.scene, prev.patch);
    const l2 = groupOf(top(world, 1))!;
    const m2 = l2.mask as Graphics;
    prev = resolveScene(scene(node({ id: 'a' })), prev.scene);
    world.apply(prev.scene, prev.patch);
    expect([l2.destroyed, m2.destroyed]).toEqual([true, true]);
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
