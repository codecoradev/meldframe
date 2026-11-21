// The browser host's text measurer, as the contract has it (`docs/fidelity-contract.md` section 6): the layout of the
// Rust core asks `(text, size, weight, wrap width)` and gets the width and height of the text in whole pixels, measured by
// the engine that draws the canvas, on the font the canvas draws with. This file is the host-independent half: the cache
// keyed as the contract keys its table, the rounding, the wait for fonts. The engine call itself (`Metrics`; Pixi's
// `CanvasTextMetrics` in the web client, apps/web/src/canvas/pixi-metrics.ts) and the font events (`FontGate`) are injected.
//
// The function it produces is the one `WasmApp.setMeasurer` takes (the Rust side is crates/app/meldframe-wasm/src/measure.rs).
// It answers NaN where it has no answer (fonts still loading, the engine threw): the app then uses its stub for that text,
// so the layout is provisional and never broken, and the NaN is never cached.
import { textMeasureShape, type TextShape } from '@meldframe/web-canvas/textstyle';

/** What the bridge calls: `wrapWidth` is null for one line with no wrapping. Returns `[width, height]`, rounded up. */
export type MeasureFn = (text: string, fontSize: number, fontWeight: number, wrapWidth: number | null) => [number, number];

/** One measurement by the engine, unrounded. `text` is never empty (an empty text is measured as a space). */
export type Metrics = (text: string, shape: TextShape) => { width: number; height: number };

/** The fonts the measurer needs: `ready` says they are loaded; `onChange` fires when that changes (loaded, or more loaded). */
export interface FontGate {
  ready(): boolean;
  onChange(listener: () => void): () => void;
}

export interface Measurer {
  measure: MeasureFn;
  /** Forgets every answer (they were measured with fonts that have changed). */
  clear(): void;
  /** How many answers are kept. */
  readonly size: number;
}

export interface MeasurerOptions {
  /** The cache holds at most this many answers; the least recently used goes first. */
  maxEntries?: number;
}

const GRID = 64;
const NO_ANSWER: [number, number] = [NaN, NaN];

export function createMeasurer(metrics: Metrics, fonts: FontGate, options: MeasurerOptions = {}): Measurer {
  const max = Math.max(1, options.maxEntries ?? 4096);
  const cache = new Map<string, [number, number]>();
  const measure: MeasureFn = (text, fontSize, fontWeight, wrapWidth) => {
    if (!fonts.ready()) return NO_ANSWER;
    // The wrap width is measured on a 1/64 px grid: a width that differs in the last bit is the same query (contract item 5).
    const wrap = wrapWidth === null ? null : Math.round(wrapWidth * GRID) / GRID;
    const key = `${fontSize}|${fontWeight}|${wrap}|${text}`;
    const hit = cache.get(key);
    if (hit) {
      cache.delete(key); // used again: last in line to go
      cache.set(key, hit);
      return hit;
    }
    let answer: [number, number];
    try {
      const m = metrics(text || ' ', textMeasureShape(fontSize, fontWeight, wrap));
      answer = [Math.ceil(m.width), Math.ceil(m.height)];
    } catch {
      return NO_ANSWER;
    }
    if (!answer.every(Number.isFinite)) return NO_ANSWER;
    if (cache.size >= max) cache.delete(cache.keys().next().value as string);
    cache.set(key, answer);
    return answer;
  };
  return { measure, clear: () => cache.clear(), get size() { return cache.size; } };
}

/**
 * Gives `measurer` to the engine, and again whenever the fonts change: the answers measured with the old ones are dropped
 * and the engine, given the measurer afresh, lays the scene out again and reads it. Returns the function that detaches it.
 */
export function attachMeasurer(engine: { setMeasurer(fn: MeasureFn): unknown }, measurer: Measurer, fonts: FontGate): () => void {
  engine.setMeasurer(measurer.measure);
  return fonts.onChange(() => {
    measurer.clear();
    engine.setMeasurer(measurer.measure);
  });
}
