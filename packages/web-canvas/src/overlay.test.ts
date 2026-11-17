import { describe, expect, it } from 'vitest';
import { rectToScreen } from './camera';
import {
  BADGE_GAP, HANDLE_SIZE, badgeOrigin, badgeText, handleAt, handlePositions, resizeCursor, selectionBox,
  touchAlpha, touchRing, visibleHandles,
} from './overlay';

describe('selection box and handles', () => {
  const rect = { x: 100, y: 50, w: 200, h: 80 };

  it('the box is the world rect through the camera and the stroke does not scale', () => {
    for (const zoom of [0.02, 0.5, 1, 4, 64]) {
      const cam = { x: 30, y: -10, zoom };
      const box = selectionBox(cam, [rect])!;
      expect(box).toEqual(rectToScreen(cam, rect));
      // handles stay HANDLE_SIZE wide whatever the zoom: their size is not a function of the camera
      expect(HANDLE_SIZE).toBe(8);
    }
  });

  it('several rectangles give their union', () => {
    const box = selectionBox({ x: 0, y: 0, zoom: 2 }, [rect, { x: 0, y: 0, w: 10, h: 10 }])!;
    expect(box).toEqual({ x: 0, y: 0, w: 600, h: 260 });
    expect(selectionBox({ x: 0, y: 0, zoom: 1 }, [])).toBeNull();
  });

  it('eight handles sit on the corners and edge midpoints', () => {
    const box = { x: 10, y: 20, w: 100, h: 60 };
    expect(handlePositions(box)).toEqual({
      nw: { x: 10, y: 20 }, n: { x: 60, y: 20 }, ne: { x: 110, y: 20 },
      e: { x: 110, y: 50 }, se: { x: 110, y: 80 }, s: { x: 60, y: 80 },
      sw: { x: 10, y: 80 }, w: { x: 10, y: 50 },
    });
  });

  it('handle positions at several zooms are the scaled corners of the world rect', () => {
    for (const zoom of [0.25, 1, 3]) {
      const cam = { x: 17, y: 9, zoom };
      const h = handlePositions(selectionBox(cam, [rect])!);
      expect(h.nw).toEqual({ x: 100 * zoom + 17, y: 50 * zoom + 9 });
      expect(h.se).toEqual({ x: 300 * zoom + 17, y: 130 * zoom + 9 });
    }
  });

  it('small boxes drop the edge handles that would touch each other', () => {
    expect(visibleHandles({ x: 0, y: 0, w: 200, h: 100 })).toHaveLength(8);
    expect(visibleHandles({ x: 0, y: 0, w: 10, h: 100 })).toEqual(expect.not.arrayContaining(['n', 's']));
    expect(visibleHandles({ x: 0, y: 0, w: 100, h: 10 })).toEqual(['nw', 'ne', 'se', 'sw']);
    expect(visibleHandles({ x: 0, y: 0, w: 1, h: 1 })).toEqual(['nw', 'ne', 'se', 'sw']);
  });

  it('handleAt finds the handle under the pointer and prefers none far away', () => {
    const box = { x: 10, y: 20, w: 100, h: 60 };
    expect(handleAt(box, 12, 22)).toBe('nw');
    expect(handleAt(box, 60, 20 - HANDLE_SIZE / 2)).toBe('n');
    expect(handleAt(box, 60, 50)).toBeNull();
    expect(handleAt({ x: 0, y: 0, w: 1, h: 1 }, 0, 0.5)).toBe('nw'); // corners win and edges are hidden
  });

  it('every handle has a resize cursor', () => {
    expect(resizeCursor('nw')).toBe('nwse-resize');
    expect(resizeCursor('s')).toBe('ns-resize');
    expect(resizeCursor('e')).toBe('ew-resize');
    expect(resizeCursor('ne')).toBe('nesw-resize');
  });
});

describe('size badge', () => {
  it('reads width times height in whole world units', () => {
    expect(badgeText(660, 77)).toBe('660 × 77');
    expect(badgeText(119.6, 61.5)).toBe('120 × 62');
    expect(badgeText(0, 0)).toBe('0 × 0');
  });

  it('is the same text at every zoom: it reports world units, not pixels', () => {
    const rect = { x: 0, y: 0, w: 660, h: 77 };
    for (const zoom of [0.02, 1, 64]) {
      const box = selectionBox({ x: 0, y: 0, zoom }, [rect])!;
      expect(badgeText(box.w / zoom, box.h / zoom)).toBe('660 × 77');
    }
  });

  it('sits centred under the box with a gap, and stays in the viewport', () => {
    const box = { x: 100, y: 100, w: 120, h: 62 };
    const vp = { w: 800, h: 600 };
    expect(badgeOrigin(box, { w: 60, h: 20 }, vp)).toEqual({ x: 130, y: 162 + BADGE_GAP });
    // near the bottom edge it moves above the box
    const low = badgeOrigin({ x: 100, y: 560, w: 120, h: 30 }, { w: 60, h: 20 }, vp);
    expect(low.y).toBeLessThan(560);
    // never off the left or right edge
    expect(badgeOrigin({ x: -50, y: 100, w: 20, h: 20 }, { w: 60, h: 20 }, vp).x).toBeGreaterThanOrEqual(0);
    expect(badgeOrigin({ x: 790, y: 100, w: 20, h: 20 }, { w: 60, h: 20 }, vp).x + 60).toBeLessThanOrEqual(800);
  });
});

describe('touch ring', () => {
  it('fades linearly over the given duration and is gone after it', () => {
    const timing = { fadeMs: 1500, holdMs: 2000 };
    const fade = timing.fadeMs;
    expect(touchAlpha(0, false, timing)).toBe(1);
    expect(touchAlpha(fade / 2, false, timing)).toBeCloseTo(0.5);
    expect(touchAlpha(fade, false, timing)).toBe(0);
    expect(touchAlpha(fade * 3, false, timing)).toBe(0);
    expect(touchAlpha(-5, false, timing)).toBe(1);
  });

  it('with reduced motion it stays fully drawn for the hold time, then goes at once', () => {
    const timing = { fadeMs: 1500, holdMs: 2000 };
    const hold = timing.holdMs;
    expect(touchAlpha(0, true, timing)).toBe(1);
    expect(touchAlpha(hold - 1, true, timing)).toBe(1);
    expect(touchAlpha(hold, true, timing)).toBe(0);
  });

  it('the ring surrounds the box and carries the label above it', () => {
    const ring = touchRing({ x: 180, y: 90, w: 100, h: 56 });
    expect(ring.rect.x).toBeLessThan(180);
    expect(ring.rect.y).toBeLessThan(90);
    expect(ring.rect.w).toBeGreaterThan(100);
    expect(ring.label.y).toBeLessThan(ring.rect.y);
    expect(ring.label.x).toBe(ring.rect.x);
    expect(ring.stroke).toBe(2);
  });
});
