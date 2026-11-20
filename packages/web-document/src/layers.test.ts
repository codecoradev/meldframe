import type { Node } from '@meldframe/protocol';
import { describe, expect, it } from 'vitest';
import { actorLabel, actorTone, layerRows } from './layers';

const node = (id: string, over: Partial<Node> = {}): Node =>
  ({ id, name: id.toUpperCase(), type: 'rect', parentId: null, x: 0, y: 0, w: 10, h: 10, pos: id, ...over }) as Node;

describe('layerRows', () => {
  const nodes = [node('a', { type: 'frame' }), node('b', { parentId: 'a', pos: 'a' }), node('c', { parentId: 'a', pos: 'b' }), node('d', { pos: 'z' })];
  // What the render step gives for these nodes: parents first, siblings by position.
  const scene = { nodes: [{ id: 'a', parent: null }, { id: 'b', parent: 'a' }, { id: 'c', parent: 'a' }, { id: 'd', parent: null }] };
  it('lists the tree with the front node first and children under their parent', () => {
    const rows = layerRows(nodes, scene, [], [], {});
    expect(rows.map((r) => [r.id, r.depth])).toEqual([['d', 0], ['a', 0], ['c', 1], ['b', 1]]);
  });
  it('marks the kind, children and the selection', () => {
    const rows = layerRows(nodes, scene, [], ['b'], {});
    expect(rows.find((r) => r.id === 'a')).toMatchObject({ kind: 'frame', hasChildren: true, selected: false, name: 'A' });
    expect(rows.find((r) => r.id === 'b')).toMatchObject({ hasChildren: false, selected: true });
  });
  it('hides the children of a collapsed node', () => {
    const rows = layerRows(nodes, scene, ['a'], [], {});
    expect(rows.map((r) => r.id)).toEqual(['d', 'a']);
    expect(rows.find((r) => r.id === 'a')?.collapsed).toBe(true);
  });
  it('carries the actor of the last change', () => {
    const rows = layerRows(nodes, scene, [], [], { b: 'agent:claude' });
    expect(rows.find((r) => r.id === 'b')?.actor).toBe('agent:claude');
    expect(rows.find((r) => r.id === 'c')?.actor).toBeNull();
  });
  it('keeps a node whose parent is missing as a root', () => {
    expect(layerRows([node('x', { parentId: 'gone' })], { nodes: [{ id: 'x', parent: null }] }, [], [], {}).map((r) => [r.id, r.depth])).toEqual([['x', 0]]);
  });
});

describe('actorLabel', () => {
  it('splits kind and name', () => {
    expect(actorLabel('agent:claude')).toEqual({ kind: 'agent', name: 'claude' });
    expect(actorLabel('person:ana')).toEqual({ kind: 'person', name: 'ana' });
  });
  it('treats an actor without a known kind as a person with that name', () => {
    expect(actorLabel('ana')).toEqual({ kind: 'person', name: 'ana' });
  });
});

describe('actorTone', () => {
  it('is stable for an actor and always one of the three agent colours', () => {
    for (const a of ['agent:a', 'agent:c', 'agent:b', 'person:ana', '']) {
      expect(actorTone(a)).toBe(actorTone(a));
      expect([1, 2, 3]).toContain(actorTone(a));
    }
  });
  it('spreads different agents over the colours', () => {
    const tones = new Set(['agent:a', 'agent:b', 'agent:c', 'agent:d', 'agent:e', 'agent:f'].map(actorTone));
    expect(tones.size).toBeGreaterThan(1);
  });
});
