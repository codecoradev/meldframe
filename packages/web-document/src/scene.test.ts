import type { Node } from '@meldframe/protocol';
import { describe, expect, it } from 'vitest';
import { parseFill, sceneFromNodes } from './scene';

const node = (over: Partial<Node> & { id: string }): Node => ({
  name: over.id, type: 'rect', parentId: null, pos: 'a0', x: 0, y: 0, w: 10, h: 10, sw: 'fixed', sh: 'fixed',
  fill: '#336699', fill2: '#000000', radius: 0, layout: 'none', gap: 0, padding: 0, align: 'start', justify: 'start',
  clip: false, text: '', fontSize: 16, shader: 'aurora', fontWeight: 400, textAlign: 'left', opacity: 1, absolute: false, marT: 0, marR: 0, marB: 0, marL: 0,
  border: { width: 0, color: '#000000', position: 'inside' }, shadows: [], ...over,
}) as Node;

describe('parseFill', () => {
  it('reads #rrggbb, #rgb and #rrggbbaa as a number and drops the alpha', () => {
    expect(parseFill('#336699')).toBe(0x336699);
    expect(parseFill('#FFF')).toBe(0xffffff);
    expect(parseFill('#33669980')).toBe(0x336699);
  });
  it('anything else is no fill', () => {
    for (const bad of ['', 'red', '#12', '#12345', 'transparent', '336699', '#gggggg']) expect(parseFill(bad), bad).toBeNull();
  });
});

describe('opacity and no fill', () => {
  it('"none" is no fill, and an opaque node carries no opacity', () => {
    const s = sceneFromNodes([node({ id: 'a', fill: 'none' }), node({ id: 'b' })]);
    expect(s.nodes[0]!.fill).toBeNull();
    expect(s.nodes[1]!.opacity).toBeUndefined();
  });
  it('a node with opacity below one carries it to the canvas', () => {
    const s = sceneFromNodes([node({ id: 'a', opacity: 0.4 }), node({ id: 'z', opacity: 0 })]);
    expect(s.nodes[0]!.opacity).toBe(0.4);
    expect(s.nodes[1]!.opacity).toBe(0);
  });
  it('a file from before opacity existed (no field) is opaque', () => {
    const old = node({ id: 'o' }) as unknown as Record<string, unknown>;
    delete old.opacity;
    expect(sceneFromNodes([old as never]).nodes[0]!.opacity).toBeUndefined();
  });
});

describe('gradient', () => {
  const stops = [{ pos: 0, color: '#ff0000' }, { pos: 1, color: '#0000ff80' }];
  it('is carried to the canvas with its kind, angle and stops', () => {
    const s = sceneFromNodes([node({ id: 'a', gradient: { kind: 'linear', angle: 45, stops } } as never)]);
    expect(s.nodes[0]!.gradient).toEqual({ kind: 'linear', angle: 45, stops });
  });
  it('a node without one carries none', () => {
    expect(sceneFromNodes([node({ id: 'a' })]).nodes[0]!.gradient).toBeUndefined();
  });
  it('stops may be #rgb, #rgba, #rrggbb or #rrggbbaa, all of which the server accepts', () => {
    const g = { kind: 'linear', angle: 0, stops: [{ pos: 0, color: '#f008' }, { pos: 1, color: '#0f0' }] };
    expect(sceneFromNodes([node({ id: 'a', gradient: g } as never)]).nodes[0]!.gradient?.stops).toHaveLength(2);
  });
  it('a gradient with a stop that is not a hex colour is left out, so the fill shows', () => {
    const bad = { kind: 'linear', angle: 0, stops: [{ pos: 0, color: 'red' }, { pos: 1, color: '#000000' }] };
    expect(sceneFromNodes([node({ id: 'a', gradient: bad } as never)]).nodes[0]!.gradient).toBeUndefined();
  });
});

describe('image', () => {
  const image = { assetId: 'c'.repeat(64), fit: 'contain' };
  it('is carried to the canvas with its asset and fit', () => {
    expect(sceneFromNodes([node({ id: 'a', image } as never)]).nodes[0]!.image).toEqual(image);
  });
  it('a node without one carries none, and text carries none even if the file names one', () => {
    expect(sceneFromNodes([node({ id: 'a' })]).nodes[0]!.image).toBeUndefined();
    expect(sceneFromNodes([node({ id: 't', type: 'text', image } as never)]).nodes[0]!.image).toBeUndefined();
  });
});

describe('sceneFromNodes', () => {
  it('maps the node kinds the canvas draws and keeps parent-relative positions', () => {
    const s = sceneFromNodes([
      node({ id: 'f', type: 'frame', clip: true, x: 5, y: 6, w: 300, h: 200 }),
      node({ id: 't', type: 'text', parentId: 'f', text: 'Hello', fontSize: 24, x: 1, y: 2 }),
    ]);
    expect(s.space).toBe('parent');
    expect(s.nodes.map((n) => [n.id, n.kind, n.parent, n.x, n.y])).toEqual([['f', 'frame', null, 5, 6], ['t', 'text', 'f', 1, 2]]);
    expect(s.nodes[0]).toMatchObject({ w: 300, h: 200, fill: 0x336699, clip: true, name: 'f' });
    expect(s.nodes[1]).toMatchObject({ text: 'Hello', fontSize: 24 });
  });

  it('draws an ellipse as a rounded rectangle and a shader as a plain rectangle', () => {
    const s = sceneFromNodes([node({ id: 'e', type: 'ellipse', w: 40, h: 20 }), node({ id: 's', type: 'shader' })]);
    expect(s.nodes[0]).toMatchObject({ kind: 'rect', radius: 10 });
    expect(s.nodes[1]).toMatchObject({ kind: 'rect' });
  });

  it('orders a parent before its children and siblings by position, whatever the order of the input', () => {
    const s = sceneFromNodes([
      node({ id: 'c2', parentId: 'f', pos: 'a2' }),
      node({ id: 'c1', parentId: 'f', pos: 'a1' }),
      node({ id: 'f', type: 'frame', pos: 'a0' }),
      node({ id: 'g', type: 'frame', pos: 'a1' }),
    ]);
    expect(s.nodes.map((n) => n.id)).toEqual(['f', 'c1', 'c2', 'g']);
  });

  it('keeps a node whose parent is missing (as a root) instead of dropping it', () => {
    const s = sceneFromNodes([node({ id: 'o', parentId: 'gone' })]);
    expect(s.nodes.map((n) => [n.id, n.parent])).toEqual([['o', null]]);
  });

  it('orders siblings by the bytes of their position, as the file does (a capital comes before a small letter)', () => {
    const s = sceneFromNodes([
      node({ id: 'n1', pos: 'aa' }), node({ id: 'n2', pos: 'aB' }), node({ id: 'n3', pos: 'a1' }), node({ id: 'n4', pos: 'aA' }),
    ]);
    expect(s.nodes.map((n) => n.id)).toEqual(['n3', 'n4', 'n2', 'n1']);
  });

  it('breaks a tie of positions by id, byte by byte', () => {
    const s = sceneFromNodes([node({ id: 'b', pos: 'a0' }), node({ id: 'B', pos: 'a0' }), node({ id: 'a', pos: 'a0' })]);
    expect(s.nodes.map((n) => n.id)).toEqual(['B', 'a', 'b']);
  });

  it('keeps nodes that only point at each other (a cycle) instead of dropping them', () => {
    const s = sceneFromNodes([node({ id: 'x', parentId: 'y' }), node({ id: 'y', parentId: 'x' })]);
    expect(s.nodes.map((n) => n.id).sort()).toEqual(['x', 'y']);
    expect(s.nodes.filter((n) => n.parent === null)).toHaveLength(1);
  });

  it('keeps the order of the list for siblings without a position, ahead of those that have one (as the file does)', () => {
    const s = sceneFromNodes([
      node({ id: 'z', pos: '' }), node({ id: 'p', pos: 'a0' }), node({ id: 'm', pos: '' }), node({ id: 'a', pos: '' }),
    ]);
    expect(s.nodes.map((n) => n.id)).toEqual(['z', 'm', 'a', 'p']);
  });
});
