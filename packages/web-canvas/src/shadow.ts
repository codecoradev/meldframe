// Shadows on the canvas: the shape a drop shadow covers, worked out as the browser draws a CSS `box-shadow` (the export
// writes one for each shadow), so the canvas and the export agree. Pure.
import type { Rect } from './geometry';

/** One drop shadow, paint only: it takes no room and lies behind the node. */
export interface SceneShadow {
  /** Offset in world units. */
  x: number;
  y: number;
  /** Blur radius in world units, 0 or more; 0 is a hard edge. */
  blur: number;
  /** Grows (above 0) or shrinks (below 0) the shadow's shape. */
  spread: number;
  /** `0xRRGGBB` (document data, not a UI token). */
  color: number;
  /** The alpha of the colour's own form, 0 to 1. */
  alpha: number;
}

/** The shadows of a node as a list in model order (the first is the bottom of the stack); none and an empty list are the same. */
export function sameShadows(a: readonly SceneShadow[] | undefined, b: readonly SceneShadow[] | undefined): boolean {
  if (a === b) return true;
  const n = a?.length ?? 0;
  if (n !== (b?.length ?? 0)) return false;
  for (let i = 0; i < n; i++) {
    const p = a![i]!;
    const q = b![i]!;
    if (p.x !== q.x || p.y !== q.y || p.blur !== q.blur || p.spread !== q.spread || p.color !== q.color || p.alpha !== q.alpha) return false;
  }
  return true;
}

/** A rounded rectangle in box coordinates (the box starts at 0, 0). */
export interface ShadowShape {
  x: number;
  y: number;
  w: number;
  h: number;
  r: number;
}

/**
 * The shape of the shadow `s` of a `w` x `h` box with corner radius `radius`, before the blur: the box grown by the
 * spread on every side (shrunk for a spread below 0) and moved by the offset, or null when nothing is left of it.
 * The radius is held to half the smaller side first, as the box is, and then follows the browser's rule for a spread:
 * a spread below 0 takes it off the radius (not below 0) and one above 0 adds to it, but a corner that is sharper than
 * the spread grows less than the spread does (the CSS ratio `1 + (r / spread - 1)^3`), so that a square corner stays
 * square. The shadow of an ellipse follows the pill that the canvas draws its fill as; the export's is a true ellipse,
 * which differs when the ellipse is not a circle.
 */
export function shadowShape(w: number, h: number, radius: number, s: SceneShadow): ShadowShape | null {
  const sw = w + 2 * s.spread;
  const sh = h + 2 * s.spread;
  if (![sw, sh, s.x, s.y, radius].every(Number.isFinite) || !(sw > 0 && sh > 0)) return null;
  const own = Math.max(0, Math.min(radius, w / 2, h / 2));
  const k = s.spread;
  let r: number;
  if (k >= 0) r = own >= k ? own + k : own + k * (1 + (own / k - 1) ** 3);
  else r = Math.max(0, own + k);
  return { x: s.x - k, y: s.y - k, w: sw, h: sh, r: Math.min(r, sw / 2, sh / 2) };
}

/**
 * The most blur, in pixels, that the canvas draws and that a capture makes room for. Pixi clips a filter to the render
 * target, whose side is at most the GPU's texture size (4096 is the least WebGL2 guarantees, and screens are less), so a
 * blur wider than 1024 px is wider than any picture the canvas draws; it is also the point at which the strength and the
 * padding of the filter (`blurFilterSettings`) and the reach of a capture box stop growing. One number for all three, so a
 * huge blur (the model allows 100000) costs a bounded filter and a bounded image. Above it the canvas blur is narrower
 * than the export's, which is not capped: a shadow with such a blur is faint over the whole of any picture anyway.
 */
export const MAX_BLUR = 1024;

/**
 * How far the blur of a shadow reaches past its shape: its radius, where the fall-off has gone to 2% of the shadow's
 * strength (a Gaussian of sigma `blur / 2`, the one CSS specifies and the canvas filter is set to match), at most
 * `MAX_BLUR`.
 */
export const blurReach = (s: SceneShadow): number => Math.min(s.blur, MAX_BLUR);

/**
 * The strength and the padding of the blur filter of a shadow of world radius `blur` when the world is scaled by `zoom`:
 * a filter works in screen pixels, so the blur on the screen is `blur * zoom`, held to `MAX_BLUR`; the strength is half of
 * it (sigma) and the padding leaves room for the tail (1.5 times the blur, and a little more).
 */
export function blurFilterSettings(blur: number, zoom: number): { strength: number; padding: number } {
  const px = Math.min(Math.max(0, blur * zoom), MAX_BLUR);
  return { strength: px / 2, padding: Math.ceil(1.5 * px) + 2 };
}

/** The rectangle every shadow of `box` covers, blur included, or null when none shows (a shadow with an alpha of 0 paints nothing). */
export function shadowBounds(box: Rect, shadows: readonly SceneShadow[] | undefined): Rect | null {
  let x0 = Infinity;
  let y0 = Infinity;
  let x1 = -Infinity;
  let y1 = -Infinity;
  for (const s of shadows ?? []) {
    if (!(s.alpha > 0)) continue;
    const shape = shadowShape(box.w, box.h, 0, s);
    if (!shape) continue;
    const g = blurReach(s);
    x0 = Math.min(x0, box.x + shape.x - g);
    y0 = Math.min(y0, box.y + shape.y - g);
    x1 = Math.max(x1, box.x + shape.x + shape.w + g);
    y1 = Math.max(y1, box.y + shape.y + shape.h + g);
  }
  return x0 > x1 ? null : { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
}
