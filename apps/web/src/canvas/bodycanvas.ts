// A node with a gradient or an image is drawn from a small 2D canvas: the fill, then the gradient over it, then the image
// over both, clipped to the rounded box. The browser's own gradient does the colours (stops, alpha), the geometry of a
// gradient is the CSS one of `gradient.ts` and the place of an image is `fitRect` of `image.ts`, so the canvas and the
// export agree. Pixi's own gradient fill mirrors a horizontal gradient, which is why it is not used.
import { gradientLine, radialRadius, type SceneGradient } from '@meldframe/web-canvas/gradient';
import { fitRect, type SceneImageFit } from '@meldframe/web-canvas/image';

/** Pixels per world unit of the texture, and the most pixels along an edge: a gradient is smooth, so little is needed. */
const SCALE = 2;
const MAX_EDGE = 2048;

/** A decoded image the painter can draw. */
export type Drawable = CanvasImageSource & { width: number; height: number };

export interface BodyPaint {
  w: number;
  h: number;
  radius: number;
  /** The node's `fill` as a CSS colour under the gradient; null for none. */
  fill: string | null;
  /** The alpha of that colour, 0 to 1; absent means 1. The gradient and the image over it have their own. */
  fillAlpha?: number;
  gradient?: SceneGradient;
  image?: { bitmap: Drawable; fit: SceneImageFit };
}

/** Draws the node into a new canvas; its size is `w * k` by `h * k` pixels for the returned `k`. */
export function paintBody(p: BodyPaint): { canvas: HTMLCanvasElement; k: number } {
  const k = Math.min(SCALE, MAX_EDGE / Math.max(p.w, p.h, 1));
  const canvas = document.createElement('canvas');
  canvas.width = Math.max(1, Math.ceil(p.w * k));
  canvas.height = Math.max(1, Math.ceil(p.h * k));
  const ctx = canvas.getContext('2d')!;
  ctx.scale(k, k);
  ctx.beginPath();
  ctx.roundRect(0, 0, p.w, p.h, Math.min(p.radius, p.w / 2, p.h / 2));
  ctx.clip();
  if (p.fill) {
    ctx.globalAlpha = p.fillAlpha ?? 1;
    ctx.fillStyle = p.fill;
    ctx.fillRect(0, 0, p.w, p.h);
    ctx.globalAlpha = 1;
  }
  const g = p.gradient;
  if (g) {
    let paint: CanvasGradient;
    if (g.kind === 'radial') {
      paint = ctx.createRadialGradient(p.w / 2, p.h / 2, 0, p.w / 2, p.h / 2, radialRadius(p.w, p.h));
    } else {
      const l = gradientLine(g.angle, p.w, p.h);
      paint = ctx.createLinearGradient(l.x0, l.y0, l.x1, l.y1);
    }
    for (const s of g.stops) paint.addColorStop(Math.min(1, Math.max(0, s.pos)), s.color);
    ctx.fillStyle = paint;
    ctx.fillRect(0, 0, p.w, p.h);
  }
  const i = p.image;
  if (i) {
    const r = fitRect(i.fit, p.w, p.h, i.bitmap.width, i.bitmap.height);
    ctx.drawImage(i.bitmap, r.x, r.y, r.w, r.h);
  }
  return { canvas, k };
}
