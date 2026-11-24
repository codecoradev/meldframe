// Image fills on the canvas: where in the node's box an image is drawn for each fit. Pure.

export type SceneImageFit = 'cover' | 'contain' | 'fill';

/** An image painted over the node's fill and gradient; `assetId` is the SHA-256 the asset is stored under. */
export interface SceneImage {
  assetId: string;
  fit: SceneImageFit;
}

export function sameImage(a: SceneImage | undefined, b: SceneImage | undefined): boolean {
  if (a === b) return true;
  if (!a || !b) return false;
  return a.assetId === b.assetId && a.fit === b.fit;
}

export interface DrawRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/**
 * The rectangle, in box coordinates (the box is `w` x `h` and starts at 0, 0), that an image of `iw` x `ih` pixels is
 * drawn into, as CSS `object-fit` does: `cover` scales it to cover the box and crops what sticks out (the box clips it),
 * `contain` scales it to fit inside the box, both centred, and `fill` stretches it to the box. An image with no size
 * gives the box itself.
 */
export function fitRect(fit: SceneImageFit, w: number, h: number, iw: number, ih: number): DrawRect {
  if (fit === 'fill' || iw <= 0 || ih <= 0) return { x: 0, y: 0, w, h };
  const s = fit === 'cover' ? Math.max(w / iw, h / ih) : Math.min(w / iw, h / ih);
  const dw = iw * s;
  const dh = ih * s;
  return { x: (w - dw) / 2, y: (h - dh) / 2, w: dw, h: dh };
}
