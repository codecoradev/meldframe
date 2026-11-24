import { describe, expect, it, vi } from 'vitest';
import { resolveScene, type Scene, type SceneNode } from '@meldframe/web-canvas/scene';
import { paintBody } from './bodycanvas';
import { WorldView } from './stage';

// A node with a gradient is drawn from a 2D canvas (`paintBody`), which a test in Node does not have: the painter and the texture are
// replaced, and the test looks at what the stage asks the painter for (the alpha of the fill's own colour, issue 234).
vi.mock('./bodycanvas', () => ({ paintBody: vi.fn(() => ({ canvas: {}, k: 1 })) }));
vi.mock('pixi.js', async (importOriginal) => {
  const pixi = await importOriginal<typeof import('pixi.js')>();
  class FakeTexture extends pixi.Texture {
    static override from() { return pixi.Texture.WHITE; }
  }
  return { ...pixi, Texture: FakeTexture };
});

const gradient = { kind: 'linear' as const, angle: 90, stops: [{ pos: 0, color: '#ff0000' }, { pos: 1, color: '#0000ff' }] };
const node = (o: Partial<SceneNode> = {}): SceneNode => ({
  id: 'g', parent: null, kind: 'rect', x: 0, y: 0, w: 100, h: 60, fill: 0x00ff00, gradient, ...o,
});
const scene = (n: SceneNode): Scene => ({ space: 'parent', nodes: [n] });

describe('the stage asks for the alpha of the fill under a gradient', () => {
  it('passes it to the painter, and draws again when only the alpha changed', () => {
    vi.mocked(paintBody).mockClear();
    const world = new WorldView();
    let prev = resolveScene(scene(node({ fillAlpha: 0.5 })));
    world.apply(prev.scene, prev.patch);
    expect(paintBody).toHaveBeenCalledTimes(1);
    expect(paintBody).toHaveBeenLastCalledWith(expect.objectContaining({ fill: '#00ff00', fillAlpha: 0.5 }));

    prev = resolveScene(scene(node({ fillAlpha: 0.25 })), prev.scene);
    world.apply(prev.scene, prev.patch);
    expect(paintBody).toHaveBeenCalledTimes(2);
    expect(paintBody).toHaveBeenLastCalledWith(expect.objectContaining({ fillAlpha: 0.25 }));

    prev = resolveScene(scene(node({ fillAlpha: 0.25 })), prev.scene);
    world.apply(prev.scene, prev.patch);
    expect(paintBody).toHaveBeenCalledTimes(2);
  });

  it('a fill with no alpha in the scene is opaque for the painter', () => {
    vi.mocked(paintBody).mockClear();
    const world = new WorldView();
    const r = resolveScene(scene(node()));
    world.apply(r.scene, r.patch);
    expect(paintBody).toHaveBeenLastCalledWith(expect.objectContaining({ fillAlpha: 1 }));
  });
});
