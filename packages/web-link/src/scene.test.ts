import { readFileSync } from 'node:fs';
import { resolveScene } from '@meldframe/web-canvas/scene';
import { describe, expect, it } from 'vitest';
import { type PaintedBox, sceneFromBoxes } from './scene';

// The file the Rust test of meldframe-render checks `resolve` against: node lists and the boxes it gives.
const fixtures: { name: string; boxes: PaintedBox[] }[] = JSON.parse(
  readFileSync(new URL('../../../crates/core/meldframe-render/tests/fixtures/scene.json', import.meta.url), 'utf8'),
);
const byName = (name: string) => fixtures.find((c) => c.name === name)!.boxes;

describe('sceneFromBoxes', () => {
  it('keeps every box of every fixture, in order, and the canvas resolves them all', () => {
    for (const c of fixtures) {
      const scene = sceneFromBoxes(c.boxes);
      expect(scene.nodes.map((n) => n.id), c.name).toEqual(c.boxes.map((b) => b.id));
      expect(resolveScene(scene).scene.items, c.name).toHaveLength(c.boxes.length);
    }
  });
  it('carries the paint, the offset in the parent, colour, opacity, gradient and image', () => {
    const nested = sceneFromBoxes(byName('nested frames, back to front')).nodes;
    expect(nested.find((n) => n.id === 'g1')).toMatchObject({ kind: 'frame', parent: 'f1', x: 20, y: 30, radius: 8, clip: false, fill: 0xeeeeee });
    // The text, its size and the name are read by the canvas too: a field renamed in Rust must fail here.
    expect(nested.find((n) => n.id === 't1')).toMatchObject({ kind: 'text', parent: 'g1', text: 'Hello', fontSize: 24, name: '' });
    expect(sceneFromBoxes(byName('opacity')).nodes.map((n) => n.opacity)).toEqual([1, 0.5, 1, 0, 0.25]);
    const paint = sceneFromBoxes(byName('gradient and image')).nodes;
    expect(paint.find((n) => n.id === 'r')).toMatchObject({
      gradient: { kind: 'linear', angle: 45, stops: [{ pos: 0, color: '#f008' }, { pos: 1, color: '#0000ff' }] },
      image: { assetId: 'ast_2', fit: 'contain' },
    });
    expect(paint.find((n) => n.id === 't')).not.toHaveProperty('gradient');
    expect(paint.find((n) => n.id === 'bad')?.gradient).toBeUndefined();
  });
  it('a missing parent and a cycle arrive as the Rust step ordered them', () => {
    expect(sceneFromBoxes(byName('a missing parent makes a root')).nodes[0]).toMatchObject({ id: 'a', parent: null });
    expect(sceneFromBoxes(byName('a cycle stays visible after the rest')).nodes.map((n) => n.id)).toEqual(['ok', 'x', 'y', 'kid']);
  });
});
