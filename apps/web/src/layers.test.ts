import { describe, expect, it } from 'vitest';
import type { Node } from '@meldframe/protocol';
import { layerRows } from './layers';

const n = (id: string, parentId: string | null, pos: string, kind: Node['type'] = 'rect', name = id): Node =>
  ({ id, name, type: kind, parentId, pos }) as Node;

describe('layerRows', () => {
  const nodes = [n('f', null, 'a', 'frame'), n('r1', 'f', 'a'), n('r2', 'f', 'b'), n('t', null, 'b', 'text')];

  it('lists the front-most sibling first and children under their parent with depth', () => {
    expect(layerRows(nodes, [], {}).map((r) => [r.id, r.depth])).toEqual([
      ['t', 0],
      ['f', 0],
      ['r2', 1],
      ['r1', 1],
    ]);
  });

  it('hides the children of a collapsed layer and marks it', () => {
    const rows = layerRows(nodes, ['f'], {});
    expect(rows.map((r) => r.id)).toEqual(['t', 'f']);
    expect(rows[1]).toMatchObject({ hasChildren: true, collapsed: true });
  });

  it('carries the actor of the last change', () => {
    const rows = layerRows(nodes, [], { r1: 'agent_x' });
    expect(rows.find((r) => r.id === 'r1')?.actor).toBe('agent_x');
    expect(rows.find((r) => r.id === 't')?.actor).toBeUndefined();
  });
});

describe('layerRows safety and cost', () => {
  it('does not show an inherited property as the actor of a node called constructor', () => {
    const rows = layerRows([n('constructor', null, 'a'), n('toString', null, 'b')], [], {});
    expect(rows.map((r) => r.actor)).toEqual([undefined, undefined]);
  });

  it('handles a large file in linear time', () => {
    const many = Array.from({ length: 20000 }, (_, i) => n(`n${i}`, i === 0 ? null : `n${Math.floor((i - 1) / 2)}`, String(i).padStart(6, '0')));
    const t = performance.now();
    expect(layerRows(many, [], {})).toHaveLength(20000);
    expect(performance.now() - t).toBeLessThan(1000);
  });
});
