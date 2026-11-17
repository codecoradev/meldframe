import { describe, expect, it } from 'vitest';
import { resolveScene, type Scene } from '@meldframe/web-canvas/scene';
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
