// The camera: a pan and a zoom as one transform, and the helpers around it. Pure: no Pixi, no DOM.
//
// screen = world * zoom + (x, y). `x` and `y` are where the world origin sits on the canvas element
// (CSS pixels, from its top-left corner), `zoom` is 1 at 100%.
import type { Point, Rect, Size } from './geometry';

export interface Camera {
  x: number;
  y: number;
  zoom: number;
}

export const ZOOM_MIN = 0.02;
export const ZOOM_MAX = 64;
/** Fit leaves this much room (CSS px) around the content. */
export const FIT_PADDING = 48;
/** A step of the plus and minus keys. */
export const ZOOM_STEP = 1.25;

export const HOME: Camera = { x: 0, y: 0, zoom: 1 };

export const clampZoom = (z: number): number => Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, z));

export const sameCamera = (a: Camera, b: Camera): boolean => a.x === b.x && a.y === b.y && a.zoom === b.zoom;

export const worldToScreen = (c: Camera, wx: number, wy: number): Point => ({ x: wx * c.zoom + c.x, y: wy * c.zoom + c.y });
export const screenToWorld = (c: Camera, sx: number, sy: number): Point => ({ x: (sx - c.x) / c.zoom, y: (sy - c.y) / c.zoom });

export const rectToScreen = (c: Camera, r: Rect): Rect => ({ x: r.x * c.zoom + c.x, y: r.y * c.zoom + c.y, w: r.w * c.zoom, h: r.h * c.zoom });
export const rectToWorld = (c: Camera, r: Rect): Rect => ({ x: (r.x - c.x) / c.zoom, y: (r.y - c.y) / c.zoom, w: r.w / c.zoom, h: r.h / c.zoom });

/** The world rectangle the viewport shows. */
export const visibleWorldRect = (c: Camera, viewport: Size): Rect => rectToWorld(c, { x: 0, y: 0, ...viewport });

export const panBy = (c: Camera, dx: number, dy: number): Camera => ({ ...c, x: c.x + dx, y: c.y + dy });

/** Sets the zoom (clamped) and keeps the world point under the screen point (sx, sy) where it is. */
export function zoomTo(c: Camera, zoom: number, sx: number, sy: number): Camera {
  const z = clampZoom(zoom);
  const wx = (sx - c.x) / c.zoom;
  const wy = (sy - c.y) / c.zoom;
  return { x: sx - wx * z, y: sy - wy * z, zoom: z };
}

/** Multiplies the zoom by `factor` around the screen point (sx, sy). */
export const zoomAt = (c: Camera, factor: number, sx: number, sy: number): Camera => zoomTo(c, c.zoom * factor, sx, sy);

export interface FitOptions {
  padding?: number;
  maxZoom?: number;
}

/** The camera that shows `r` centred in the viewport; the home camera, centred, for no rectangle. */
export function fitRect(r: Rect | null, viewport: Size, opts: FitOptions = {}): Camera {
  const pad = opts.padding ?? FIT_PADDING;
  if (!r || r.w <= 0 || r.h <= 0) {
    const zoom = 1;
    const cx = r ? r.x : 0;
    const cy = r ? r.y : 0;
    return { x: viewport.w / 2 - cx * zoom, y: viewport.h / 2 - cy * zoom, zoom };
  }
  const availW = Math.max(1, viewport.w - 2 * pad);
  const availH = Math.max(1, viewport.h - 2 * pad);
  const zoom = Math.min(clampZoom(Math.min(availW / r.w, availH / r.h)), opts.maxZoom ?? ZOOM_MAX);
  return {
    x: viewport.w / 2 - (r.x + r.w / 2) * zoom,
    y: viewport.h / 2 - (r.y + r.h / 2) * zoom,
    zoom,
  };
}

/** The zoom as the percentage the status readout shows (100 at zoom 1). */
export const zoomPercent = (zoom: number): number => Math.round(zoom * 100);
export const formatZoom = (zoom: number): string => `${zoomPercent(zoom)}%`;

export interface WheelIn {
  deltaX: number;
  deltaY: number;
  /** 0 pixels, 1 lines, 2 pages (WheelEvent.deltaMode). */
  deltaMode: number;
  ctrlKey: boolean;
  metaKey: boolean;
}

const LINE_PX = 16;
/** One wheel tick of a mouse zooms by about this much at most: the delta is limited so a notch is a step. */
const WHEEL_ZOOM_CLAMP = 40;
const WHEEL_ZOOM_RATE = 0.01;

export type WheelAction = { kind: 'pan'; dx: number; dy: number } | { kind: 'zoom'; factor: number };

/**
 * What a wheel event means. The wheel pans (two-finger scroll on a trackpad too); with Ctrl or Meta held it
 * zooms, which is also how browsers report a trackpad pinch (a wheel event with ctrlKey set).
 */
export function wheelAction(e: WheelIn, viewport: Size): WheelAction {
  const unit = e.deltaMode === 1 ? LINE_PX : e.deltaMode === 2 ? viewport.h : 1;
  const dx = e.deltaX * unit;
  const dy = e.deltaY * unit;
  if (e.ctrlKey || e.metaKey) {
    const d = Math.max(-WHEEL_ZOOM_CLAMP, Math.min(WHEEL_ZOOM_CLAMP, dy));
    return { kind: 'zoom', factor: Math.exp(-d * WHEEL_ZOOM_RATE) };
  }
  return { kind: 'pan', dx: -dx, dy: -dy };
}

/** Applies a wheel event at the screen point (sx, sy). */
export function applyWheel(c: Camera, e: WheelIn, sx: number, sy: number, viewport: Size): Camera {
  const a = wheelAction(e, viewport);
  return a.kind === 'pan' ? panBy(c, a.dx, a.dy) : zoomAt(c, a.factor, sx, sy);
}

export type Pair = readonly [Point, Point];

const mid = (p: Pair): Point => ({ x: (p[0].x + p[1].x) / 2, y: (p[0].y + p[1].y) / 2 });
const dist = (p: Pair): number => Math.hypot(p[0].x - p[1].x, p[0].y - p[1].y);

/**
 * A two-pointer gesture from the pair `prev` to the pair `next`: the zoom follows the change in distance, and
 * the world point that was under the old midpoint ends under the new one (so a pinch also pans).
 */
export function pinch(c: Camera, prev: Pair, next: Pair): Camera {
  const d0 = dist(prev);
  if (d0 < 1e-6) return c;
  const m0 = mid(prev);
  const m1 = mid(next);
  const zoom = clampZoom(c.zoom * (dist(next) / d0));
  const wx = (m0.x - c.x) / c.zoom;
  const wy = (m0.y - c.y) / c.zoom;
  return { x: m1.x - wx * zoom, y: m1.y - wy * zoom, zoom };
}
