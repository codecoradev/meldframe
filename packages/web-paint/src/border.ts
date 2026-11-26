// Borders on the canvas: the ring a border covers, worked out as the export writes it (an `::after` of `inset: 0 / -w/2 / -w`
// with a `border-radius` that grows by the share of the width outside the box), so the canvas and the export agree. Pure.

export type SceneBorderPosition = 'inside' | 'center' | 'outside';

/** A uniform border, paint only: it takes no room and lies over the node's children. */
export interface SceneBorder {
  /** Stroke width in world units, above 0. */
  width: number;
  /** `0xRRGGBB` (document data, not a UI token). */
  color: number;
  /** The alpha of the colour's own form, 0 to 1. */
  alpha: number;
  position: SceneBorderPosition;
}

export function sameBorder(a: SceneBorder | undefined, b: SceneBorder | undefined): boolean {
  if (a === b) return true;
  if (!a || !b) return false;
  return a.width === b.width && a.color === b.color && a.alpha === b.alpha && a.position === b.position;
}

/** A rounded rectangle in box coordinates (the box starts at 0, 0). */
export interface RingShape {
  x: number;
  y: number;
  w: number;
  h: number;
  r: number;
}

/**
 * The two edges of the border of a `w` x `h` box with corner radius `radius`: the ring is the outer shape less the inner one.
 * `inner` is null when nothing is left inside the border (an inside border as wide as half of the box).
 * An inside border has the box for its outer edge, an outside border for its inner edge, and a centred one straddles it.
 * The radius is held to half the smaller side first, as the box is; the outer radius grows by the share of the width that
 * lies outside the box (not when the radius is 0: a square corner stays square) and the inner one is the outer less the
 * width, never below 0.
 *
 * The outer shape is the border box of the export's `::after`, so a border wider than half its box makes it bigger than
 * the box, as CSS does: the content of a box is not below 0, so the border box is at least twice the width on each axis (a
 * box with no height gets a ring as high as both of its borders, from its top). A radius is the ring of a pill for an ellipse
 * too: the export draws a non-square ellipse with `border-radius: 50%`, a true ellipse, and the canvas draws its fill as a
 * pill, so the ring follows the fill and the two differ for an ellipse that is not a circle.
 */
export function borderRing(w: number, h: number, radius: number, b: SceneBorder): { outer: RingShape; inner: RingShape | null } {
  const grow = b.position === 'inside' ? 0 : b.position === 'center' ? b.width / 2 : b.width;
  const own = Math.min(radius, w / 2, h / 2);
  const r = own > 0 ? own + grow : 0;
  const outer: RingShape = { x: 0 - grow, y: 0 - grow, w: Math.max(w + 2 * grow, 2 * b.width), h: Math.max(h + 2 * grow, 2 * b.width), r };
  const iw = outer.w - 2 * b.width;
  const ih = outer.h - 2 * b.width;
  if (iw <= 0 || ih <= 0) return { outer, inner: null };
  return { outer, inner: { x: outer.x + b.width, y: outer.y + b.width, w: iw, h: ih, r: Math.max(0, r - b.width) } };
}
