import { describe, expect, it, vi } from 'vitest';
import { attachMeasurer, createMeasurer, type FontGate, type Metrics } from './measurer';

/** A font gate the test opens and changes by hand. */
function gate(ready = true): FontGate & { set(v: boolean): void; change(): void; listeners: number } {
  const fns = new Set<() => void>();
  let on = ready;
  return {
    ready: () => on,
    onChange: (fn) => { fns.add(fn); return () => fns.delete(fn); },
    set: (v) => { on = v; },
    change: () => fns.forEach((f) => f()),
    get listeners() { return fns.size; },
  };
}

/** Metrics that answer 7.3 px a character and 17.2 px a line, and count their calls. */
const counting = () => {
  const calls: Parameters<Metrics>[] = [];
  const metrics: Metrics = (text, shape) => {
    calls.push([text, shape]);
    return { width: text.length * 7.3, height: 17.2 };
  };
  return { metrics, calls };
};

describe('createMeasurer', () => {
  it('answers the width and the height rounded up, as the measure table of the contract does', () => {
    const { metrics } = counting();
    const m = createMeasurer(metrics, gate());
    expect(m.measure('Hello', 16, 400, null)).toEqual([37, 18]); // 36.5 and 17.2
  });

  it('measures an empty text as a space, like the canvas and the stub', () => {
    const { metrics, calls } = counting();
    createMeasurer(metrics, gate()).measure('', 16, 400, null);
    expect(calls[0]![0]).toBe(' ');
  });

  it('asks for the shape the canvas draws with: size at least 1, wrap width at least 1, null for one line', () => {
    const { metrics, calls } = counting();
    const m = createMeasurer(metrics, gate());
    m.measure('a', 0, 700, null);
    m.measure('b', 12, 300, 0.2);
    expect(calls.map((c) => c[1])).toEqual([
      { fontSize: 1, fontWeight: 700, wrapWidth: null },
      { fontSize: 12, fontWeight: 300, wrapWidth: 1 },
    ]);
  });

  it('remembers an answer: the same text, size, weight and wrap width is measured once', () => {
    const { metrics, calls } = counting();
    const m = createMeasurer(metrics, gate());
    m.measure('Hello', 16, 400, 100);
    m.measure('Hello', 16, 400, 100);
    expect(calls).toHaveLength(1);
    expect(m.size).toBe(1);
  });

  it('keys on the weight, the size, the wrap width and the text', () => {
    const { metrics, calls } = counting();
    const m = createMeasurer(metrics, gate());
    m.measure('Hello', 16, 400, 100);
    m.measure('Hello', 16, 700, 100); // weight
    m.measure('Hello', 17, 400, 100); // size
    m.measure('Hello', 16, 400, 120); // wrap width
    m.measure('Hello', 16, 400, null); // no wrapping
    m.measure('Hello!', 16, 400, 100); // text
    expect(calls).toHaveLength(6);
    expect(m.size).toBe(6);
  });

  it('puts wrap widths on a 1/64 px grid, so that a last-bit difference is the same query (contract section 6, item 5)', () => {
    const { metrics, calls } = counting();
    const m = createMeasurer(metrics, gate());
    m.measure('Hello', 16, 400, 119.33333333333333);
    m.measure('Hello', 16, 400, 119.33333333333334);
    expect(calls).toHaveLength(1);
    expect(calls[0]![1].wrapWidth).toBe(Math.round(119.33333333333333 * 64) / 64);
    m.measure('Hello', 16, 400, 119.4);
    expect(calls).toHaveLength(2);
  });

  it('is bounded: past its limit the least recently used answer goes first', () => {
    const { metrics, calls } = counting();
    const m = createMeasurer(metrics, gate(), { maxEntries: 3 });
    for (const t of ['a', 'b', 'c']) m.measure(t, 16, 400, null);
    m.measure('a', 16, 400, null); // a is used again: b is the oldest now
    m.measure('d', 16, 400, null); // evicts b
    expect(m.size).toBe(3);
    const before = calls.length;
    m.measure('a', 16, 400, null);
    m.measure('c', 16, 400, null);
    m.measure('d', 16, 400, null);
    expect(calls.length).toBe(before); // still there
    m.measure('b', 16, 400, null);
    expect(calls.length).toBe(before + 1); // measured again
  });

  it('gives no answer while the fonts are not ready, and keeps nothing of it', () => {
    const { metrics, calls } = counting();
    const g = gate(false);
    const m = createMeasurer(metrics, g);
    const [w, h] = m.measure('Hello', 16, 400, null);
    expect([w, h].every(Number.isNaN)).toBe(true); // the app falls back to the stub for a NaN
    expect(calls).toHaveLength(0);
    expect(m.size).toBe(0);
    g.set(true);
    expect(m.measure('Hello', 16, 400, null)).toEqual([37, 18]);
  });

  it('gives no answer, and keeps nothing, when the metrics throw or give a number that is not a size', () => {
    for (const bad of [() => { throw new Error('no canvas'); }, () => ({ width: NaN, height: 10 }), () => ({ width: 10, height: Infinity })]) {
      const m = createMeasurer(bad as Metrics, gate());
      expect(m.measure('x', 16, 400, null).every(Number.isNaN)).toBe(true);
      expect(m.size).toBe(0);
    }
  });

  it('forgets everything on clear', () => {
    const { metrics, calls } = counting();
    const m = createMeasurer(metrics, gate());
    m.measure('Hello', 16, 400, null);
    m.clear();
    expect(m.size).toBe(0);
    m.measure('Hello', 16, 400, null);
    expect(calls).toHaveLength(2);
  });
});

describe('attachMeasurer', () => {
  const engine = () => ({ setMeasurer: vi.fn() });

  it('hands the measurer to the engine at once', () => {
    const e = engine();
    const m = createMeasurer(counting().metrics, gate());
    attachMeasurer(e, m, gate());
    expect(e.setMeasurer).toHaveBeenCalledTimes(1);
    expect(e.setMeasurer).toHaveBeenCalledWith(m.measure);
  });

  it('measures again when the fonts change: the answers of the old fonts go and the engine asks again', () => {
    const e = engine();
    const g = gate(false);
    const { metrics } = counting();
    const m = createMeasurer(metrics, g);
    attachMeasurer(e, m, g);
    expect(m.measure('Hello', 16, 400, null).every(Number.isNaN)).toBe(true);
    g.set(true);
    m.measure('Hello', 16, 400, null); // measured and kept
    expect(m.size).toBe(1);
    g.change();
    expect(m.size).toBe(0);
    expect(e.setMeasurer).toHaveBeenCalledTimes(2);
  });

  it('stops listening when it is detached', () => {
    const e = engine();
    const g = gate();
    const detach = attachMeasurer(e, createMeasurer(counting().metrics, g), g);
    expect(g.listeners).toBe(1);
    detach();
    expect(g.listeners).toBe(0);
    g.change();
    expect(e.setMeasurer).toHaveBeenCalledTimes(1);
  });
});
