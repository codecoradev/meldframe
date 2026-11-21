// What decides how a text is drawn and how large it measures: one definition, so that the drawing and the measurer cannot
// drift apart (a text measured at 400 and drawn at 700 would wrap where the layout did not expect it). No Pixi here: the
// canvas turns a shape into a Pixi style, the host's measurer into a call to Pixi's `CanvasTextMetrics`.
import type { SceneTextAlign } from './scene';

/** The facts of a text that its size depends on (the key of the measure table, `docs/fidelity-contract.md` section 6, without the text). */
export interface TextShape {
  /** Never below 1 px. */
  fontSize: number;
  /** 100 to 900. */
  fontWeight: number;
  /** The width the lines are wrapped at; null for one line, no wrapping. */
  wrapWidth: number | null;
}

/** The shape the canvas draws a text node with: wrapped at the width of its box, one line when the box has no width. */
export function textDrawShape(n: { fontSize: number; fontWeight: number; w: number }): TextShape {
  return textMeasureShape(n.fontSize, n.fontWeight, n.w);
}

/**
 * The shape to measure with when the layout asks for `wrapWidth`: the one `textDrawShape` gives a box of that width. A width that is
 * not above 0 (or null) is one line with no wrapping, as the canvas has always drawn a box with no width; a width below 1 px wraps at 1.
 */
export function textMeasureShape(fontSize: number, fontWeight: number, wrapWidth: number | null): TextShape {
  return { fontSize: Math.max(1, fontSize), fontWeight, wrapWidth: wrapWidth !== null && wrapWidth > 0 ? Math.max(1, wrapWidth) : null };
}

/** Where the left edge of a block of lines `textW` wide goes in a box `boxW` wide. A block wider than the box starts at its left edge. */
export function alignOffset(align: SceneTextAlign, boxW: number, textW: number): number {
  const room = Math.max(0, boxW - textW);
  return align === 'center' ? room / 2 : align === 'right' ? room : 0;
}

/** What a field laid over a text must carry, besides the size (which the zoom scales where the field is placed), to look like the text the canvas draws. */
export interface TextFieldStyle {
  /** CSS `font-weight`: 100 to 900. */
  fontWeight: number;
  /** CSS `text-align`. */
  textAlign: SceneTextAlign;
}

const ALIGNS: readonly SceneTextAlign[] = ['left', 'center', 'right'];

/**
 * The weight and the alignment a text is drawn with, for the field that edits it: the drawing's own values (the resolved ones the
 * scene carries), with a weight outside 100 to 900 or not a number taken as regular (400) and an unknown alignment as left, which is
 * what the canvas shows for them.
 */
export function textFieldStyle(n: { fontWeight?: number; textAlign?: SceneTextAlign }): TextFieldStyle {
  const w = n.fontWeight;
  const weight = typeof w === 'number' && Number.isFinite(w) && w >= 100 && w <= 900 ? Math.round(w / 100) * 100 : 400;
  return { fontWeight: weight, textAlign: n.textAlign && ALIGNS.includes(n.textAlign) ? n.textAlign : 'left' };
}
