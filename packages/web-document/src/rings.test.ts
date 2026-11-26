import { describe, expect, it } from 'vitest';
import type { TouchRing } from '@meldframe/protocol';
import { actorView } from './fixtures';
import { ringsOnCanvas } from './rings';

const palette = { person: 1, agent1: 10, agent2: 20, agent3: 30 };
// The app layer stamps a ring with the epoch milliseconds of the shell's clock; the canvas fades by performance.now().
const clock = { epochMs: 1_000_000, perfMs: 5_000 };
const ring = (over: Partial<TouchRing> = {}): TouchRing => ({ actor: 'agent:claude', view: actorView('agent:claude', 'agent2'), ids: ['a', 'b'], atMs: 1_000_000, persistent: false, ...over });

describe('ringsOnCanvas', () => {
  it('draws a ring of the state in the colour of its actor view tone, labelled with its name, on its nodes', () => {
    const [t] = ringsOnCanvas([ring()], clock, palette);
    expect(t).toMatchObject({ color: 20, label: 'claude', nodeIds: ['a', 'b'] });
  });

  it('maps each tone to its colour and a person to the person colour, and hashes nothing', () => {
    const out = (view: ReturnType<typeof actorView>) => ringsOnCanvas([ring({ actor: view.actor, view })], clock, palette)[0]!.color;
    expect(out(actorView('person:ana'))).toBe(1);
    expect([out(actorView('agent:a', 'agent1')), out(actorView('agent:a', 'agent2')), out(actorView('agent:a', 'agent3'))]).toEqual([10, 20, 30]);
    // The colour follows the view the app layer sent, whatever the characters of the id.
    expect(out(actorView('agent:\u{1F300}', 'agent3'))).toBe(30);
  });

  it('puts a ring on the canvas clock by its age, so the fade runs from when it came and not from when it is drawn', () => {
    const [young, old] = ringsOnCanvas([ring({ atMs: 1_000_000 }), ring({ atMs: 999_000, ids: ['c'] })], clock, palette);
    expect(young!.at).toBe(5_000);
    expect(old!.at).toBe(4_000);
  });

  it('never puts a ring in the future when the two clocks disagree', () => {
    expect(ringsOnCanvas([ring({ atMs: 1_000_500 })], clock, palette)[0]!.at).toBe(5_000);
  });

  it('keeps a persistent ring at full strength for as long as the state holds it', () => {
    const [t] = ringsOnCanvas([ring({ persistent: true })], clock, palette);
    expect(t!.at).toBe(Number.POSITIVE_INFINITY);
  });

  it('leaves out a ring with no nodes, and gives every ring its own id', () => {
    const out = ringsOnCanvas([ring({ ids: [] }), ring(), ring({ persistent: true })], clock, palette);
    expect(out).toHaveLength(2);
    expect(new Set(out.map((t) => t.id)).size).toBe(2);
  });

  it('copies the ids, so a later change of the state does not move a ring already handed over', () => {
    const r = ring();
    const [t] = ringsOnCanvas([r], clock, palette);
    r.ids.push('z');
    expect(t!.nodeIds).toEqual(['a', 'b']);
  });
});
