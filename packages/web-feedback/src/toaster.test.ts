import { describe, expect, it } from 'vitest';
import { Toaster, type ToastView } from './toaster';

// A clock the test drives by hand: `set` queues, `advance` runs what is due.
function clock() {
  let now = 0;
  const due: { at: number; fn: () => void; id: number }[] = [];
  let next = 1;
  return {
    timers: {
      set: (fn: () => void, ms: number) => {
        due.push({ at: now + ms, fn, id: next });
        return next++;
      },
      clear: (t: unknown) => {
        const i = due.findIndex((d) => d.id === t);
        if (i >= 0) due.splice(i, 1);
      },
    },
    advance(ms: number) {
      now += ms;
      for (const d of due.filter((x) => x.at <= now)) {
        due.splice(due.indexOf(d), 1);
        d.fn();
      }
    },
    pending: () => due.length,
  };
}

function setup(ms = 6000) {
  const c = clock();
  const seen: (ToastView | null)[] = [];
  const t = new Toaster((v) => seen.push(v), ms, c.timers);
  return { c, seen, t };
}

describe('the toast', () => {
  it('shows a message at once and takes it away after its time', () => {
    const { c, seen, t } = setup();
    t.show('That node no longer exists.');
    expect(seen.at(-1)).toMatchObject({ text: 'That node no longer exists.', tone: 'warning' });
    c.advance(5999);
    expect(seen.at(-1)).not.toBeNull();
    c.advance(1);
    expect(seen.at(-1)).toBeNull();
  });

  it('a newer message replaces the one on screen and starts its own time', () => {
    const { c, seen, t } = setup();
    t.show('first');
    c.advance(4000);
    t.show('second', 'danger');
    c.advance(4000); // 8 s after the first, 4 s after the second: the first timer must not take the second away
    expect(seen.at(-1)).toMatchObject({ text: 'second', tone: 'danger' });
    c.advance(2000);
    expect(seen.at(-1)).toBeNull();
  });

  it('the same text said twice is a new toast (a new id), so a reader hears it again', () => {
    const { seen, t } = setup();
    t.show('same');
    t.show('same');
    const ids = seen.filter((v): v is ToastView => v !== null).map((v) => v.id);
    expect(new Set(ids).size).toBe(2);
  });

  it('dismiss takes it away at once and leaves no timer behind', () => {
    const { c, seen, t } = setup();
    t.show('x');
    t.dismiss();
    expect(seen.at(-1)).toBeNull();
    expect(c.pending()).toBe(0);
  });

  it('destroy stops the timer without telling anyone (the page is going away)', () => {
    const { c, seen, t } = setup();
    t.show('x');
    const calls = seen.length;
    t.destroy();
    expect(c.pending()).toBe(0);
    expect(seen.length).toBe(calls);
  });
});
