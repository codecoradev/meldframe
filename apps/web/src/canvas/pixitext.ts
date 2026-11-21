// How the canvas draws text with Pixi: one style for the drawing and for a host's measurement, so that a box the layout sized
// from a measurement is wide enough for the text the canvas then draws in it.
import type { ResolvedNode } from '@meldframe/web-canvas/scene';
import { textDrawShape, type TextShape } from '@meldframe/web-canvas/textstyle';
import { fonts } from '@meldframe/web-tokens/tokens';

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
