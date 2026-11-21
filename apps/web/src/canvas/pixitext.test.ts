import { describe, expect, it, vi } from 'vitest';
import { fontSize, fontWeight, fonts } from '@meldframe/web-tokens/tokens';
import { browserFontGate, canvasLayoutRequested, drawStyle, installPixiMeasurer, pixiTextStyle } from './pixitext';

describe('pixiTextStyle', () => {
  it('draws with the font stack of the page, the size, the weight as the string Pixi takes, and the wrap of the shape', () => {
    expect(pixiTextStyle({ fontSize: 20, fontWeight: 700, wrapWidth: 120 })).toEqual({
      fontFamily: fonts.sans, fontSize: 20, fontWeight: '700', wordWrap: true, wordWrapWidth: 120,
    });
  });
  it('one line is no wrap at all', () => {
    expect(pixiTextStyle({ fontSize: 12, fontWeight: 400, wrapWidth: null })).toMatchObject({ wordWrap: false });
  });
});

describe('drawStyle', () => {
  const node = { fontSize: 18, fontWeight: 600, textAlign: 'center', w: 140, fill: 0x112233 } as const;
  it('is the style the measurer measures with, plus the fill and the alignment', () => {
    expect(drawStyle(node)).toEqual({
      fontFamily: fonts.sans, fontSize: 18, fontWeight: '600', wordWrap: true, wordWrapWidth: 140, fill: 0x112233, align: 'center',
    });
  });
  it('draws a text with no fill in black, and one with no width on one line', () => {
    expect(drawStyle({ ...node, fill: null, w: 0 })).toMatchObject({ fill: 0, wordWrap: false });
  });
});

/** A `document.fonts` the test drives: `load` resolves when the test says. */
function fakeFonts() {
  const listeners = new Map<string, Set<() => void>>();
  let finish: () => void = () => {};
  const fonts = {
    load: vi.fn(() => new Promise<unknown[]>((res) => { finish = () => res([]); })),
    addEventListener: (t: string, fn: () => void) => { (listeners.get(t) ?? listeners.set(t, new Set()).get(t)!).add(fn); },
    removeEventListener: (t: string, fn: () => void) => { listeners.get(t)?.delete(fn); },
  };
  return { doc: { fonts } as unknown as Document, fonts, finish: () => finish(), fire: (t: string) => listeners.get(t)?.forEach((f) => f()), count: (t: string) => listeners.get(t)?.size ?? 0 };
}
const tick = () => new Promise((r) => setTimeout(r, 0));

describe('browserFontGate', () => {
  it('asks for the font of the canvas and is not ready until it arrives', async () => {
    const f = fakeFonts();
    const gate = browserFontGate(f.doc);
    expect(f.fonts.load).toHaveBeenCalledWith(`${fontWeight.regular} ${fontSize.title}px ${fonts.sans}`);
    expect(gate.ready()).toBe(false);
    const seen = vi.fn();
    gate.onChange(seen);
    f.finish();
    await tick();
    expect(gate.ready()).toBe(true);
    expect(seen).toHaveBeenCalledTimes(1);
  });

  it('is ready at once, and never changes, where the document has no font loading API', () => {
    const gate = browserFontGate({} as Document);
    expect(gate.ready()).toBe(true);
    const seen = vi.fn();
    gate.onChange(seen)();
    expect(seen).not.toHaveBeenCalled();
  });

  it('is ready when the font failed to load too: the canvas draws the fallback, and so should the layout measure it', async () => {
    const f = fakeFonts();
    f.fonts.load.mockImplementation(() => Promise.reject(new Error('404')));
    const gate = browserFontGate(f.doc);
    await tick();
    expect(gate.ready()).toBe(true);
  });

  it('tells again when another face finishes loading later (a script the first draw did not need)', async () => {
    const f = fakeFonts();
    const gate = browserFontGate(f.doc);
    const seen = vi.fn();
    gate.onChange(seen);
    f.fire('loadingdone'); // before the first load is over: nothing to tell yet
    expect(seen).not.toHaveBeenCalled();
    f.finish();
    await tick();
    f.fire('loadingdone');
    expect(seen).toHaveBeenCalledTimes(2);
  });

  it('listens to the document only while somebody listens to it', async () => {
    const f = fakeFonts();
    const gate = browserFontGate(f.doc);
    expect(f.count('loadingdone')).toBe(0);
    const off = gate.onChange(() => {});
    expect(f.count('loadingdone')).toBe(1);
    off();
    expect(f.count('loadingdone')).toBe(0);
  });
});

describe('installPixiMeasurer', () => {
  it('gives the engine a measurer at once and takes it away when asked', () => {
    const f = fakeFonts();
    const engine = { setMeasurer: vi.fn() };
    const off = installPixiMeasurer(engine, f.doc);
    expect(engine.setMeasurer).toHaveBeenCalledTimes(1);
    expect(engine.setMeasurer.mock.calls[0]![0]('x', 16, 400, null).every(Number.isNaN)).toBe(true); // fonts not loaded yet
    off();
    expect(f.count('loadingdone')).toBe(0);
  });

  it('gives it again when the font has arrived, so that the scene is laid out with it', async () => {
    const f = fakeFonts();
    const engine = { setMeasurer: vi.fn() };
    installPixiMeasurer(engine, f.doc);
    f.finish();
    await tick();
    expect(engine.setMeasurer).toHaveBeenCalledTimes(2);
  });
});

describe('canvasLayoutRequested', () => {
  it('is the development switch ?canvaslayout=1 and nothing else: off by default', () => {
    expect(canvasLayoutRequested('?canvaslayout=1')).toBe(true);
    expect(canvasLayoutRequested('?tokens&canvaslayout=1')).toBe(true);
    for (const q of ['', '?', '?canvaslayout', '?canvaslayout=0', '?canvaslayout=true', '?layout=1', '?x=canvaslayout=1']) expect(canvasLayoutRequested(q), q).toBe(false);
  });
});
