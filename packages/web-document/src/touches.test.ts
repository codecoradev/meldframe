import { describe, expect, it } from 'vitest';
import { actorColor, addTouch, type Touched } from './touches';

const palette = { person: 1, agents: [10, 20, 30] };
const opts = { self: 'agent:me', palette, maxAgeMs: 2000 };

describe('actorColor', () => {
  it('gives a person the person colour and an agent one of the agent colours, the same one every time', () => {
    expect(actorColor('person:ana', palette)).toBe(1);
    const c = actorColor('agent:claude', palette);
    expect([10, 20, 30]).toContain(c);
    expect(actorColor('agent:claude', palette)).toBe(c);
  });
});

describe('addTouch', () => {
  it('adds a ring labelled with the name of the actor', () => {
    const list = addTouch([], { actor: 'agent:claude', nodeIds: ['a'] }, 100, opts);
    expect(list).toHaveLength(1);
    expect(list[0]).toMatchObject({ label: 'claude', nodeIds: ['a'], at: 100 });
  });
  it('ignores the touches of this page and touches without nodes', () => {
    expect(addTouch([], { actor: 'agent:me', nodeIds: ['a'] }, 0, opts)).toEqual([]);
    expect(addTouch([], { actor: 'agent:x', nodeIds: [] }, 0, opts)).toEqual([]);
  });
  it('drops rings older than the maximum age and keeps the newer ones', () => {
    let list: Touched[] = [];
    list = addTouch(list, { actor: 'agent:a', nodeIds: ['1'] }, 0, opts);
    list = addTouch(list, { actor: 'agent:b', nodeIds: ['2'] }, 1500, opts);
    list = addTouch(list, { actor: 'agent:c', nodeIds: ['3'] }, 2500, opts);
    expect(list.map((t) => t.label)).toEqual(['b', 'c']);
  });
  it('keeps at most a bounded number of rings, newest last', () => {
    let list: Touched[] = [];
    for (let i = 0; i < 100; i++) list = addTouch(list, { actor: 'agent:a', nodeIds: [String(i)] }, i, { ...opts, maxAgeMs: 1e9 });
    expect(list.length).toBeLessThanOrEqual(50);
    expect(list.at(-1)?.nodeIds).toEqual(['99']);
  });
  it('gives every ring its own id', () => {
    let list: Touched[] = [];
    list = addTouch(list, { actor: 'agent:a', nodeIds: ['1'] }, 5, opts);
    list = addTouch(list, { actor: 'agent:a', nodeIds: ['1'] }, 5, opts);
    expect(new Set(list.map((t) => t.id)).size).toBe(2);
  });
});
