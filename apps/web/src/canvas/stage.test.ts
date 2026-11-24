import { describe, expect, it } from 'vitest';
import { Container, Graphics, Sprite } from 'pixi.js';
import { resolveScene, type Scene, type SceneNode } from '@meldframe/web-canvas/scene';
import { WorldView } from './stage';

const frame = (id: string, parent: string | null) => ({
  id, parent, kind: 'frame' as const, x: 0, y: 0, w: 100, h: 100, fill: 0x808080,
});
const scene = (...nodes: ReturnType<typeof frame>[]): Scene => ({ space: 'parent', nodes });

describe('WorldView patches', () => {
  it('a child moved to the canvas in the same patch that removes its parent survives', () => {
    const world = new WorldView();
    const first = resolveScene(scene(frame('f', null), frame('c', 'f')));
    world.apply(first.scene, first.patch);
    expect(world.nodeCount).toBe(2);

    // The host deletes frame f and promotes its child c to the canvas in one change.
    const second = resolveScene(scene(frame('c', null)), first.scene);
    expect(second.patch.removed).toEqual(['f']);
    expect(second.patch.changed).toEqual(['c']);
    expect(() => world.apply(second.scene, second.patch)).not.toThrow();
    expect(world.nodeCount).toBe(1);
    expect(world.root.children).toHaveLength(1);
    expect(world.root.children[0]?.destroyed).toBe(false);
  });

  it('removing a parent and all its children together leaves nothing behind', () => {
    const world = new WorldView();
    const first = resolveScene(scene(frame('f', null), frame('c', 'f')));
    world.apply(first.scene, first.patch);
    const second = resolveScene(scene(), first.scene);
    world.apply(second.scene, second.patch);
    expect(world.nodeCount).toBe(0);
    expect(world.root.children).toHaveLength(0);
  });
});

// The alpha of a fill is the node's own colour alpha (issue 234), not its `opacity`: the body takes it, the container keeps opacity.
describe('WorldView draws the alpha of a fill', () => {
  const draw = (o: Partial<SceneNode>) => {
    const world = new WorldView();
    const r = resolveScene({ space: 'parent', nodes: [{ ...frame('f', null), ...o }] });
    world.apply(r.scene, r.patch);
    const container = world.root.children[0] as Container;
    return { container, body: container.children[0] as Sprite | Graphics };
  };

  it('a plain box is a tinted sprite with that alpha, and the container keeps the opacity', () => {
    const { container, body } = draw({ fillAlpha: 0.5, opacity: 0.8 });
    expect(body).toBeInstanceOf(Sprite);
    expect(body.alpha).toBe(0.5);
    expect((body as Sprite).tint).toBe(0x808080);
    expect(container.alpha).toBe(0.8);
  });

  it('an opaque fill draws at alpha 1, and a fully transparent one at 0', () => {
    expect(draw({}).body.alpha).toBe(1);
    expect(draw({ fillAlpha: 0 }).body.alpha).toBe(0);
  });

  it('a rounded box fills its path with that alpha', () => {
    const { body } = draw({ radius: 8, fillAlpha: 0.25 });
    expect(body).toBeInstanceOf(Graphics);
    const fill = (body as Graphics).context.instructions.find((i) => i.action === 'fill');
    expect((fill?.data as { style: { alpha: number; color: number } }).style).toMatchObject({ alpha: 0.25, color: 0x808080 });
  });

  it('a change of the alpha alone draws the body again', () => {
    const world = new WorldView();
    const first = resolveScene({ space: 'parent', nodes: [{ ...frame('f', null), fillAlpha: 1 }] });
    world.apply(first.scene, first.patch);
    const second = resolveScene({ space: 'parent', nodes: [{ ...frame('f', null), fillAlpha: 0.5 }] }, first.scene);
    world.apply(second.scene, second.patch);
    expect(((world.root.children[0] as Container).children[0] as Sprite).alpha).toBe(0.5);
  });
});
