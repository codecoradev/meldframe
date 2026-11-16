import { describe, expect, it } from 'vitest';
import {
  ZOOM_MAX, ZOOM_MIN, applyWheel, clampZoom, fitRect, formatZoom, panBy, pinch, rectToScreen, rectToWorld,
  screenToWorld, visibleWorldRect, wheelAction, worldToScreen, zoomAt, zoomPercent, zoomTo, type Camera,
} from './camera';

/** A small seeded generator so a failure can be replayed. */
function rng(seed: number) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
const between = (r: () => number, lo: number, hi: number) => lo + (hi - lo) * r();
const randomCamera = (r: () => number): Camera => ({
  x: between(r, -5000, 5000),
  y: between(r, -5000, 5000),
  zoom: Math.exp(between(r, Math.log(ZOOM_MIN), Math.log(ZOOM_MAX))),
});
const close = (a: number, b: number, tol = 1e-6) => expect(Math.abs(a - b)).toBeLessThanOrEqual(tol * Math.max(1, Math.abs(a), Math.abs(b)));

describe('transforms', () => {
  it('world -> screen -> world is the identity for random cameras and points', () => {
    const r = rng(1);
    for (let i = 0; i < 500; i++) {
      const c = randomCamera(r);
      const wx = between(r, -1e5, 1e5);
      const wy = between(r, -1e5, 1e5);
      const s = worldToScreen(c, wx, wy);
      const w = screenToWorld(c, s.x, s.y);
      close(w.x, wx);
      close(w.y, wy);
    }
  });

  it('screen -> world -> screen is the identity', () => {
    const r = rng(2);
    for (let i = 0; i < 500; i++) {
      const c = randomCamera(r);
      const sx = between(r, 0, 2000);
      const sy = between(r, 0, 1200);
      const w = screenToWorld(c, sx, sy);
      const s = worldToScreen(c, w.x, w.y);
      close(s.x, sx);
      close(s.y, sy);
    }
  });

  it('rectangles round trip and scale by the zoom', () => {
    const c: Camera = { x: 10, y: -20, zoom: 2 };
    const s = rectToScreen(c, { x: 5, y: 5, w: 30, h: 10 });
    expect(s).toEqual({ x: 20, y: -10, w: 60, h: 20 });
    expect(rectToWorld(c, s)).toEqual({ x: 5, y: 5, w: 30, h: 10 });
  });

  it('visibleWorldRect is the viewport seen through the camera', () => {
    expect(visibleWorldRect({ x: 100, y: 50, zoom: 0.5 }, { w: 400, h: 200 })).toEqual({ x: -200, y: -100, w: 800, h: 400 });
  });
});

describe('pan and zoom', () => {
  it('panBy moves the screen position and keeps the zoom', () => {
    expect(panBy({ x: 1, y: 2, zoom: 3 }, 10, -5)).toEqual({ x: 11, y: -3, zoom: 3 });
  });

  it('zoomAt keeps the world point under the cursor fixed (random cameras, cursors and factors)', () => {
    const r = rng(3);
    for (let i = 0; i < 500; i++) {
      const c = randomCamera(r);
      const sx = between(r, 0, 2000);
      const sy = between(r, 0, 1200);
      const before = screenToWorld(c, sx, sy);
      const next = zoomAt(c, Math.exp(between(r, -2, 2)), sx, sy);
      const after = screenToWorld(next, sx, sy);
      close(after.x, before.x, 1e-6);
      close(after.y, before.y, 1e-6);
    }
  });

  it('zoom is limited to 2% and 6400%', () => {
    expect(clampZoom(0.0001)).toBe(ZOOM_MIN);
    expect(clampZoom(1000)).toBe(ZOOM_MAX);
    expect(ZOOM_MIN).toBe(0.02);
    expect(ZOOM_MAX).toBe(64);
    const c = zoomTo({ x: 0, y: 0, zoom: 1 }, 1e9, 100, 100);
    expect(c.zoom).toBe(ZOOM_MAX);
    // the anchor stays put even when the limit cut the factor
    const w = screenToWorld(c, 100, 100);
    close(w.x, 100);
  });

  it('zooming in then out by the same factor returns to the start', () => {
    const r = rng(4);
    for (let i = 0; i < 200; i++) {
      const c: Camera = { ...randomCamera(r), zoom: between(r, 0.2, 5) };
      const sx = between(r, 0, 1000);
      const sy = between(r, 0, 800);
      const there = zoomAt(c, 1.5, sx, sy);
      const back = zoomAt(there, 1 / 1.5, sx, sy);
      close(back.zoom, c.zoom);
      close(back.x, c.x);
      close(back.y, c.y);
    }
  });
});

describe('fit to content', () => {
  const viewport = { w: 1000, h: 600 };
  it('centres the content and leaves the padding', () => {
    const c = fitRect({ x: 100, y: 100, w: 500, h: 200 }, viewport, { padding: 50 });
    expect(c.zoom).toBeCloseTo(1.8); // width: 900 / 500
    const s = rectToScreen(c, { x: 100, y: 100, w: 500, h: 200 });
    close(s.x + s.w / 2, 500);
    close(s.y + s.h / 2, 300);
    expect(s.x).toBeGreaterThanOrEqual(50 - 1e-9);
    expect(s.x + s.w).toBeLessThanOrEqual(950 + 1e-9);
  });

  it('always shows the whole rectangle (random rectangles)', () => {
    const r = rng(5);
    for (let i = 0; i < 300; i++) {
      const rect = { x: between(r, -1e4, 1e4), y: between(r, -1e4, 1e4), w: between(r, 1, 1e4), h: between(r, 1, 1e4) };
      const c = fitRect(rect, viewport, { padding: 20 });
      const s = rectToScreen(c, rect);
      if (c.zoom > ZOOM_MIN && c.zoom < ZOOM_MAX) {
        expect(s.x).toBeGreaterThanOrEqual(20 - 1e-6);
        expect(s.y).toBeGreaterThanOrEqual(20 - 1e-6);
        expect(s.x + s.w).toBeLessThanOrEqual(viewport.w - 20 + 1e-6);
        expect(s.y + s.h).toBeLessThanOrEqual(viewport.h - 20 + 1e-6);
      }
    }
  });

  it('honours a zoom cap and the limits', () => {
    expect(fitRect({ x: 0, y: 0, w: 10, h: 10 }, viewport, { maxZoom: 1 }).zoom).toBe(1);
    expect(fitRect({ x: 0, y: 0, w: 1e9, h: 1e9 }, viewport).zoom).toBe(ZOOM_MIN);
  });

  it('centres the origin at zoom 100% for no content', () => {
    expect(fitRect(null, viewport)).toEqual({ x: 500, y: 300, zoom: 1 });
  });
});

describe('wheel and pinch', () => {
  const vp = { w: 800, h: 600 };
  const wheel = (o: Partial<{ deltaX: number; deltaY: number; deltaMode: number; ctrlKey: boolean; metaKey: boolean }>) =>
    ({ deltaX: 0, deltaY: 0, deltaMode: 0, ctrlKey: false, metaKey: false, ...o });

  it('a plain wheel pans against the delta', () => {
    expect(wheelAction(wheel({ deltaX: 5, deltaY: 30 }), vp)).toEqual({ kind: 'pan', dx: -5, dy: -30 });
  });

  it('line and page deltas are converted to pixels', () => {
    expect(wheelAction(wheel({ deltaY: 3, deltaMode: 1 }), vp)).toEqual({ kind: 'pan', dx: -0, dy: -48 });
    expect(wheelAction(wheel({ deltaY: 1, deltaMode: 2 }), vp)).toEqual({ kind: 'pan', dx: -0, dy: -600 });
  });

  it('ctrl or meta zooms: up zooms in, down zooms out, one notch is limited', () => {
    const inn = wheelAction(wheel({ deltaY: -10, ctrlKey: true }), vp);
    const out = wheelAction(wheel({ deltaY: 10, metaKey: true }), vp);
    expect(inn.kind === 'zoom' && inn.factor).toBeGreaterThan(1);
    expect(out.kind === 'zoom' && out.factor).toBeLessThan(1);
    const huge = wheelAction(wheel({ deltaY: -5000, ctrlKey: true }), vp);
    expect(huge.kind === 'zoom' && huge.factor).toBeLessThan(2);
  });

  it('applyWheel zoom keeps the cursor point fixed', () => {
    const c: Camera = { x: 30, y: 40, zoom: 1.3 };
    const before = screenToWorld(c, 200, 150);
    const next = applyWheel(c, wheel({ deltaY: -20, ctrlKey: true }), 200, 150, vp);
    const after = screenToWorld(next, 200, 150);
    close(after.x, before.x);
    close(after.y, before.y);
    expect(next.zoom).toBeGreaterThan(c.zoom);
  });

  it('pinch keeps the world points under both fingers under them (random gestures)', () => {
    const r = rng(6);
    for (let i = 0; i < 300; i++) {
      const c: Camera = { ...randomCamera(r), zoom: between(r, 0.1, 8) };
      const a0 = { x: between(r, 0, 800), y: between(r, 0, 600) };
      const b0 = { x: a0.x + between(r, 50, 300), y: a0.y + between(r, 50, 300) };
      const k = between(r, 0.5, 2);
      // a pure scale around the midpoint plus a shift: both fingers' world points must follow
      const m = { x: (a0.x + b0.x) / 2, y: (a0.y + b0.y) / 2 };
      const shift = { x: between(r, -100, 100), y: between(r, -100, 100) };
      const a1 = { x: m.x + (a0.x - m.x) * k + shift.x, y: m.y + (a0.y - m.y) * k + shift.y };
      const b1 = { x: m.x + (b0.x - m.x) * k + shift.x, y: m.y + (b0.y - m.y) * k + shift.y };
      const next = pinch(c, [a0, b0], [a1, b1]);
      if (next.zoom <= ZOOM_MIN || next.zoom >= ZOOM_MAX) continue;
      const wa = screenToWorld(c, a0.x, a0.y);
      const wb = screenToWorld(c, b0.x, b0.y);
      const sa = worldToScreen(next, wa.x, wa.y);
      const sb = worldToScreen(next, wb.x, wb.y);
      close(sa.x, a1.x, 1e-6);
      close(sa.y, a1.y, 1e-6);
      close(sb.x, b1.x, 1e-6);
      close(sb.y, b1.y, 1e-6);
    }
  });

  it('pinch with coincident fingers changes nothing and the zoom stays within limits', () => {
    const c: Camera = { x: 1, y: 2, zoom: 1 };
    const p = { x: 5, y: 5 };
    expect(pinch(c, [p, p], [p, { x: 9, y: 9 }])).toBe(c);
    const far = pinch({ x: 0, y: 0, zoom: 60 }, [{ x: 0, y: 0 }, { x: 10, y: 0 }], [{ x: 0, y: 0 }, { x: 1000, y: 0 }]);
    expect(far.zoom).toBe(ZOOM_MAX);
  });
});

describe('zoom readout', () => {
  it('shows a whole percentage', () => {
    expect(zoomPercent(1)).toBe(100);
    expect(formatZoom(0.05)).toBe('5%');
    expect(formatZoom(0.02)).toBe('2%');
    expect(formatZoom(64)).toBe('6400%');
    expect(formatZoom(1.234)).toBe('123%');
  });
});
