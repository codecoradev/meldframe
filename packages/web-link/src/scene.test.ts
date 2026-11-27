import { readFileSync } from 'node:fs';
import { captureBounds, captureRectOf, resolveScene } from '@meldframe/web-canvas/scene';
import { MAX_BLUR } from '@meldframe/web-paint/shadow';
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
    // The weight and the alignment are drawn by the canvas too (E3.1 coverage table: rendered).
    expect(nested.find((n) => n.id === 't1')).toMatchObject({ fontWeight: 400, textAlign: 'left' });
    expect(sceneFromBoxes(byName('text weight and alignment')).nodes.find((n) => n.id === 't')).toMatchObject({ fontWeight: 700, textAlign: 'center' });
    expect(sceneFromBoxes(byName('opacity')).nodes.map((n) => n.opacity)).toEqual([1, 0.5, 1, 0, 0.25]);
    const paint = sceneFromBoxes(byName('gradient and image')).nodes;
    expect(paint.find((n) => n.id === 'r')).toMatchObject({
      gradient: { kind: 'linear', angle: 45, stops: [{ pos: 0, color: '#f008' }, { pos: 1, color: '#0000ff' }] },
      image: { assetId: 'ast_2', fit: 'contain' },
    });
    expect(paint.find((n) => n.id === 't')).not.toHaveProperty('gradient');
    expect(paint.find((n) => n.id === 'bad')?.gradient).toBeUndefined();
  });
  it('carries the alpha of a fill next to its colour (issue 234)', () => {
    const fills = sceneFromBoxes(byName('fill colours')).nodes;
    expect(fills.find((n) => n.id === 'c')).toMatchObject({ fill: 0x112233, fillAlpha: 0x44 / 255 });
    expect(fills.find((n) => n.id === 'k')).toMatchObject({ fill: 0xff8800, fillAlpha: 0xcc / 255 });
    expect(fills.find((n) => n.id === 'a')).toMatchObject({ fill: 0xff8000, fillAlpha: 1 });
  });
  it('carries the border of a box as the render step painted it, and none for a box without one', () => {
    const nodes = sceneFromBoxes(byName('border')).nodes;
    expect(nodes.find((n) => n.id === 'f')?.border).toEqual({ width: 6, color: 0xff0000, alpha: 1, position: 'inside' });
    expect(nodes.find((n) => n.id === 'r')?.border).toEqual({ width: 6, color: 0x0000ff, alpha: 1, position: 'outside' });
    expect(nodes.find((n) => n.id === 'e')?.border).toEqual({ width: 6, color: 0x00ff00, alpha: 0x80 / 255, position: 'center' });
    // A text, a zero width and a box with no border arrive without one.
    for (const id of ['t', 'z']) expect(nodes.find((n) => n.id === id), id).not.toHaveProperty('border');
    expect(sceneFromBoxes(byName('opacity')).nodes.every((n) => !('border' in n))).toBe(true);
  });
  it('carries the shadows of a box as the render step painted them, in order, and none for a text or a box without any', () => {
    const nodes = sceneFromBoxes(byName('shadows')).nodes;
    expect(nodes.find((n) => n.id === 'f')?.shadows).toEqual([
      { x: 0, y: 4, blur: 12, spread: 0, color: 0, alpha: 0x40 / 255 },
      { x: 2, y: -2, blur: 0, spread: 3, color: 0xff0000, alpha: 0x80 / 255 },
    ]);
    // The negative blur is a hard edge and the colour that is not hex is gone.
    expect(nodes.find((n) => n.id === 'r')?.shadows).toEqual([{ x: 0, y: 0, blur: 0, spread: 1, color: 0x0000ff, alpha: 1 }]);
    expect(nodes.find((n) => n.id === 'e')?.shadows).toEqual([{ x: 3, y: 3, blur: 0, spread: -2, color: 0xff8800, alpha: 1 }]);
    expect(nodes.find((n) => n.id === 't')).not.toHaveProperty('shadows');
    expect(sceneFromBoxes(byName('border')).nodes.every((n) => !('shadows' in n))).toBe(true);
  });
  it('carries what a box paints beyond its rectangle, as the render step worked it out, and none for a box that paints inside it', () => {
    const border = sceneFromBoxes(byName('border')).nodes;
    expect(border.find((n) => n.id === 'r')?.paintBounds).toEqual({ left: 6, top: 6, right: 6, bottom: 6 });
    expect(border.find((n) => n.id === 'e')?.paintBounds).toEqual({ left: 3, top: 3, right: 3, bottom: 3 });
    for (const id of ['f', 't', 'z']) expect(border.find((n) => n.id === id), id).not.toHaveProperty('paintBounds');
    const shadows = sceneFromBoxes(byName('shadows')).nodes;
    expect(shadows.find((n) => n.id === 'f')?.paintBounds).toEqual({ left: 12, top: 8, right: 12, bottom: 16 });
    expect(shadows.find((n) => n.id === 't')).not.toHaveProperty('paintBounds');
    expect(sceneFromBoxes(byName('opacity')).nodes.every((n) => !('paintBounds' in n))).toBe(true);
  });
  it('the capture box of the fixtures is the one the TypeScript reach gave before it moved to the render step', () => {
    const rs = resolveScene(sceneFromBoxes(byName('paint reach'))).scene;
    // The numbers the capture tests of web-canvas and the TypeScript reach (`borderReach` and `shadowBounds`, now removed from
    // web-paint) gave before the reach moved to the render step: a shadow with a blur, a border and a shadow together, a shadow
    // that leaves nothing, a shadow with no alpha, a shadow of a child that goes out of its parent.
    expect(captureRectOf(rs, 'a')).toEqual({ x: 90, y: 94, w: 70, h: 60 });
    expect(captureRectOf(rs, 'b')).toEqual({ x: 294, y: 94, w: 78, h: 52 });
    expect(captureRectOf(rs, 'c')).toEqual({ x: 500, y: 100, w: 50, h: 40 });
    expect(captureRectOf(rs, 'z')).toEqual({ x: 500, y: 400, w: 10, h: 10 });
    expect(captureRectOf(rs, 'k')).toEqual({ x: 890, y: 100, w: 50, h: 20 });
    // The cap of the reach is the cap of the blur the canvas draws: a blur of 1e9 reaches MAX_BLUR on every side.
    expect(captureRectOf(rs, 'h')).toEqual({ x: 500 - MAX_BLUR, y: 300 - MAX_BLUR, w: 10 + 2 * MAX_BLUR, h: 10 + 2 * MAX_BLUR });
    // The page: `h` is the widest, on both sides.
    const page = captureBounds(rs)!;
    expect(page.x).toBe(500 - MAX_BLUR);
    expect(page.x + page.w).toBe(500 + 10 + MAX_BLUR);
  });
  it('a missing parent and a cycle arrive as the Rust step ordered them', () => {
    expect(sceneFromBoxes(byName('a missing parent makes a root')).nodes[0]).toMatchObject({ id: 'a', parent: null });
    expect(sceneFromBoxes(byName('a cycle stays visible after the rest')).nodes.map((n) => n.id)).toEqual(['ok', 'x', 'y', 'kid']);
  });
});
