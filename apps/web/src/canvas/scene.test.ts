import { describe, expect, it } from 'vitest';
import type { Node } from '@meldframe/protocol';
import { hitTest, parentAt, sceneItems } from './scene';

const n = (o: Partial<Node> & { id: string }): Node =>
  ({ name: o.id, type: 'rect', parentId: null, pos: 'a', x: 0, y: 0, w: 10, h: 10, ...o }) as Node;

describe('scene', () => {
  const nodes = [
    n({ id: 'f', type: 'frame', x: 100, y: 100, w: 200, h: 200 }),
    n({ id: 'r', parentId: 'f', x: 10, y: 20, w: 50, h: 50 }),
    n({ id: 'a', x: 0, y: 0, w: 20, h: 20, pos: 'b' }),
  ];

  it('places children in their parent space and draws back to front', () => {
    const items = sceneItems(nodes);
    expect(items.map((i) => i.id)).toEqual(['f', 'r', 'a']);
    expect(items.find((i) => i.id === 'r')).toMatchObject({ x: 110, y: 120, w: 50, h: 50 });
  });

  it('hits the front-most node under a point', () => {
    const items = sceneItems(nodes);
    expect(hitTest(items, 120, 130)).toBe('r');
    expect(hitTest(items, 250, 250)).toBe('f');
    expect(hitTest(items, 5, 5)).toBe('a');
    expect(hitTest(items, 900, 900)).toBeNull();
  });

  it('finds the frame a new node lands in and gives its local point', () => {
    const items = sceneItems(nodes);
    expect(parentAt(items, 150, 160)).toEqual({ parent: 'f', x: 50, y: 60 });
    expect(parentAt(items, 900, 900)).toEqual({ parent: null, x: 900, y: 900 });
  });
});
