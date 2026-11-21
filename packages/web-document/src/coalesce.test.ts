import { describe, expect, it } from 'vitest';
import { perFrame, type Frames } from './coalesce';

/** A clock of animation frames the test turns by hand. */
function fakeFrames() {
  let next = 1;
  const waiting = new Map<number, () => void>();
  const frames: Frames = {
    request: (cb) => {
      const id = next++;
      waiting.set(id, cb);
      return id;
    },
    cancel: (id) => void waiting.delete(id),
  };
  const turn = () => {
    const cbs = [...waiting.values()];
    waiting.clear();
    cbs.forEach((cb) => cb());
  };
  return { frames, turn, waiting: () => waiting.size };
}

describe('perFrame', () => {
  it('runs once per frame with the latest value, however many arrive between two frames', () => {
    const f = fakeFrames();
    const seen: number[] = [];
    const c = perFrame<number>(f.frames, (v) => seen.push(v));
    c.push(1);
    c.push(2);
    c.push(3);
    expect(seen).toEqual([]);
    expect(f.waiting()).toBe(1);
    f.turn();
    expect(seen).toEqual([3]);
    f.turn();
    expect(seen).toEqual([3]);
    c.push(4);
    f.turn();
    expect(seen).toEqual([3, 4]);
  });

  it('flush runs the waiting value now and leaves nothing for the frame', () => {
    const f = fakeFrames();
    const seen: number[] = [];
    const c = perFrame<number>(f.frames, (v) => seen.push(v));
    c.push(1);
    c.push(2);
    c.flush();
    expect(seen).toEqual([2]);
    expect(f.waiting()).toBe(0);
    c.flush();
    expect(seen).toEqual([2]);
  });

  it('cancel drops the waiting value', () => {
    const f = fakeFrames();
    const seen: number[] = [];
    const c = perFrame<number>(f.frames, (v) => seen.push(v));
    c.push(1);
    c.cancel();
    f.turn();
    expect(seen).toEqual([]);
    expect(f.waiting()).toBe(0);
    c.push(5);
    f.turn();
    expect(seen).toEqual([5]);
  });
});
