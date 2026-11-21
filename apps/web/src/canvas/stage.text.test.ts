import { describe, expect, it, vi } from 'vitest';
import { resolveScene, type Scene, type SceneNode } from '@meldframe/web-canvas/scene';
import { alignOffset } from '@meldframe/web-canvas/textstyle';
import { WorldView } from './stage';

// Pixi measures a text with a canvas, which a test in Node does not have: the one thing it asks of a Text here, the width of
// its block of lines, is a number the test picks (BLOCK_WIDTH); everything else of Pixi is the real one.
const BLOCK_WIDTH = 80;
vi.mock('pixi.js', async (importOriginal) => {
  const pixi = await importOriginal<typeof import('pixi.js')>();
  class FakeText extends pixi.Container {
    text: string;
    style: unknown;
    constructor(o: { text: string; style: unknown }) {
      super();
      this.text = o.text;
      this.style = o.style;
    }
    override get width(): number { return BLOCK_WIDTH; }
    override set width(_v: number) {}
  }
  return { ...pixi, Text: FakeText };
});

type Style = { fontWeight: string; align: string; wordWrap: boolean; wordWrapWidth: number };
const text = (o: Partial<SceneNode> = {}): SceneNode => ({
  id: 't', parent: null, kind: 'text', x: 0, y: 0, w: 200, h: 40, fill: 0x112233, text: 'Hello', fontSize: 20, ...o,
});
const scene = (n: SceneNode): Scene => ({ space: 'parent', nodes: [n] });

/** Draws the scenes one after the other, as the host patches the stage, and gives the Text of the node. */
function draw(...nodes: SceneNode[]) {
  const world = new WorldView();
  let prev = resolveScene(scene(nodes[0]!));
  world.apply(prev.scene, prev.patch);
  for (const n of nodes.slice(1)) {
    prev = resolveScene(scene(n), prev.scene);
    world.apply(prev.scene, prev.patch);
  }
  const found: { x: number; style: Style }[] = [];
  const walk = (o: { children?: unknown[] }) => o.children?.forEach((c) => { if ((c as { text?: unknown }).text === 'Hello') found.push(c as never); else walk(c as never); });
  walk(world.root);
  expect(found).toHaveLength(1);
  return found[0]!;
}

describe('the stage draws the weight and the alignment of a text', () => {
  it('draws with the weight and the wrap of the node', () => {
    const t = draw(text({ fontWeight: 700 }));
    expect(t.style).toMatchObject({ fontWeight: '700', wordWrap: true, wordWrapWidth: 200, align: 'left' });
  });

  it('draws the text again when only the weight changed', () => {
    const t = draw(text({ fontWeight: 400 }), text({ fontWeight: 700 }));
    expect(t.style.fontWeight).toBe('700');
  });

  it('draws the text again when only the alignment changed, and moves the block with it', () => {
    const t = draw(text({ textAlign: 'left' }), text({ textAlign: 'center' }));
    expect(t.style.align).toBe('center');
    expect(t.x).toBe(60);
  });

  it('puts the block of a wrapped text at the left, the middle or the right of its box (alignOffset)', () => {
    for (const [align, x] of [['left', 0], ['center', 60], ['right', 120]] as const) {
      const t = draw(text({ textAlign: align }));
      expect(t.x, align).toBe(x);
      expect(t.x).toBe(alignOffset(align, 200, BLOCK_WIDTH));
    }
  });
});
