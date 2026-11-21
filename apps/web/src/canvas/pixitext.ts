// How the canvas draws text with Pixi and how the browser host measures it: one style for both, so that a box the layout
// sized from a measurement is wide enough for the text the canvas then draws in it. The host-independent half of the
// measurer (cache, rounding, wait for fonts) is `@meldframe/web-link/measurer`; this is its Pixi and DOM half.
import { CanvasTextMetrics, TextStyle } from 'pixi.js';
import { attachMeasurer, createMeasurer, type FontGate, type Metrics } from '@meldframe/web-link/measurer';
import type { ResolvedNode } from '@meldframe/web-canvas/scene';
import { textDrawShape, type TextShape } from '@meldframe/web-canvas/textstyle';
import { fontSize, fontWeight, fonts } from '@meldframe/web-tokens/tokens';

/** The Pixi style of a text of this shape (the fill and the alignment are the drawing's own). */
export function pixiTextStyle(shape: TextShape) {
  return {
    fontFamily: fonts.sans,
    fontSize: shape.fontSize,
    fontWeight: String(shape.fontWeight) as never, // Pixi types the weight as '100' to '900'
    wordWrap: shape.wrapWidth !== null,
    wordWrapWidth: shape.wrapWidth ?? 0,
  };
}

/** The whole style a text node is drawn with: what the measurer measures with, the fill, and where the lines sit in the block. */
export function drawStyle(n: Pick<ResolvedNode, 'fontSize' | 'fontWeight' | 'textAlign' | 'w' | 'fill'>) {
  return { ...pixiTextStyle(textDrawShape(n)), fill: n.fill ?? 0, align: n.textAlign };
}

/** Pixi's own measurement of a text, on the style the canvas draws it with (the oracle's table is recorded the same way). */
export const pixiMetrics: Metrics = (text, shape) => {
  const m = CanvasTextMetrics.measureText(text, new TextStyle(pixiTextStyle(shape)));
  return { width: m.width, height: m.height };
};

/** The face the canvas draws with (the variable Inter file serves every weight): asking for it makes the browser load it. */
const FONT_TO_LOAD = `${fontWeight.regular} ${fontSize.title}px ${fonts.sans}`;

/**
 * The fonts of the page as a gate: ready once the face the canvas draws with has loaded (or failed to: the canvas then draws
 * the fallback, and the layout should measure it), and tells again whenever another face finishes loading.
 */
export function browserFontGate(doc: Document = document): FontGate {
  const listeners = new Set<() => void>();
  // A page with no font loading API has nothing to wait for: the fonts are as loaded as they will be.
  if (!doc.fonts) return { ready: () => true, onChange: () => () => {} };
  let loaded = false;
  const tell = () => listeners.forEach((fn) => fn());
  const onDone = () => { if (loaded) tell(); };
  const settled = () => { loaded = true; tell(); };
  void doc.fonts.load(FONT_TO_LOAD).then(settled, settled);
  return {
    ready: () => loaded,
    onChange(fn) {
      if (!listeners.size) doc.fonts.addEventListener('loadingdone', onDone);
      listeners.add(fn);
      return () => {
        listeners.delete(fn);
        if (!listeners.size) doc.fonts.removeEventListener('loadingdone', onDone);
      };
    },
  };
}

/** Gives the engine a measurer backed by Pixi's text metrics on the page's fonts; returns the function that stops it following the fonts. */
export function installPixiMeasurer(engine: Parameters<typeof attachMeasurer>[0], doc: Document = document): () => void {
  const gate = browserFontGate(doc);
  return attachMeasurer(engine, createMeasurer(pixiMetrics, gate), gate);
}

/** The development switch `/?canvaslayout=1` (off by default, not offered in the UI): draw laid-out boxes, measured as above. */
export const canvasLayoutRequested = (search: string): boolean => new URLSearchParams(search).get('canvaslayout') === '1';
