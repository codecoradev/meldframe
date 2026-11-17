// A node with a gradient is drawn from a small 2D canvas: the fill, then the gradient over it, clipped to the rounded box.
// The browser's own gradient does the colours (stops, alpha), and the geometry is the CSS one of `gradient.ts`, so the
// canvas and the export agree. Pixi's own gradient fill mirrors a horizontal gradient, which is why it is not used.
import { gradientLine, radialRadius, type SceneGradient } from '@meldframe/web-canvas/gradient';

/** Pixels per world unit of the texture, and the most pixels along an edge: a gradient is smooth, so little is needed. */
const SCALE = 2;
const MAX_EDGE = 2048;

export interface GradientPaint {
  w: number;
  h: number;
  radius: number;
  /** The node's `fill` as a CSS colour under the gradient; null for none. */
  fill: string | null;
  gradient: SceneGradient;
}

/** Draws the node into a new canvas; its size is `w * k` by `h * k` pixels for the returned `k`. */
export function paintGradient(p: GradientPaint): { canvas: HTMLCanvasElement; k: number } {
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
    ctx.fillStyle = p.fill;
    ctx.fillRect(0, 0, p.w, p.h);
  }
  const g = p.gradient;
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
  return { canvas, k };
}
