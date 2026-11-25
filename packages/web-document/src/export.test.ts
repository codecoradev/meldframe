import { describe, expect, it } from 'vitest';
import { EXPORT_MAX_PIXELS, EXPORT_SCALES, exportFileName, exportScaleLabel, exportSize, largestFittingScale, planExport } from './export';

describe('exportFileName', () => {
  it('adds `@<scale>x` to the stem the app layer gave, for every scale but 1x', () => {
    expect(exportFileName('Hero', 1)).toBe('Hero.png');
    expect(exportFileName('Hero', 2)).toBe('Hero@2x.png');
    expect(exportFileName('Hero', 3)).toBe('Hero@3x.png');
    expect(exportFileName('Hero', 4)).toBe('Hero@4x.png');
    // Below 1x the suffix is there too, so a half-size file is never mistaken for the 1x file.
    expect(exportFileName('Hero', 0.5)).toBe('Hero@0.5x.png');
  });
  it('has one suffix rule for every scale the editor offers', () => {
    for (const s of EXPORT_SCALES) expect(exportFileName('A', s)).toBe(s === 1 ? 'A.png' : `A@${s}x.png`);
  });
  it('takes the extension of the HTML of a frame', () => {
    expect(exportFileName('Landing Desktop', 1, 'html')).toBe('Landing Desktop.html');
  });
  it('cleans nothing: the stem is the app layer\'s (its rules are tested in meldframe-app-export-tests, file_names.rs)', () => {
    expect(exportFileName('a/b', 1)).toBe('a/b.png');
  });
});

describe('planExport', () => {
  it('offers 0.5x, 1x, 2x, 3x and 4x', () => {
    expect(EXPORT_SCALES).toEqual([0.5, 1, 2, 3, 4]);
    expect(EXPORT_SCALES.map(exportScaleLabel)).toEqual(['0.5x', '1x', '2x', '3x', '4x']);
  });
  it('gives the pixels of the image at each scale', () => {
    const box = { x: 0, y: 0, w: 600, h: 400 };
    expect(EXPORT_SCALES.map((s) => planExport(box, s))).toEqual([
      { ok: true, scale: 0.5, width: 300, height: 200 },
      { ok: true, scale: 1, width: 600, height: 400 },
      { ok: true, scale: 2, width: 1200, height: 800 },
      { ok: true, scale: 3, width: 1800, height: 1200 },
      { ok: true, scale: 4, width: 2400, height: 1600 },
    ]);
  });
  it('rounds each side up to a whole pixel, as the drawing does', () => {
    expect(planExport({ x: 0, y: 0, w: 100.5, h: 50 }, 2)).toEqual({ ok: true, scale: 2, width: 201, height: 100 });
    // 101 x 0.5 = 50.5 and 33 x 0.5 = 16.5: both go up, never down, so no edge of the frame is cut.
    expect(planExport({ x: 0, y: 0, w: 101, h: 33 }, 0.5)).toEqual({ ok: true, scale: 0.5, width: 51, height: 17 });
    expect(planExport({ x: 0, y: 0, w: 100.34, h: 10 }, 3)).toEqual({ ok: true, scale: 3, width: 302, height: 30 });
    // A box under one pixel at 0.5x is still one pixel, not none.
    expect(planExport({ x: 0, y: 0, w: 1, h: 1 }, 0.5)).toEqual({ ok: true, scale: 0.5, width: 1, height: 1 });
  });
  it('refuses a node that is not there or too large', () => {
    expect(planExport(null, 1)).toEqual({ ok: false, code: 'node_not_found' });
    const side = Math.sqrt(EXPORT_MAX_PIXELS);
    expect(planExport({ x: 0, y: 0, w: side, h: side }, 1).ok).toBe(true);
    expect(planExport({ x: 0, y: 0, w: side, h: side }, 2)).toEqual({ ok: false, code: 'too_large' });
    expect(planExport({ x: 0, y: 0, w: side, h: side }, 0.5).ok).toBe(true);
  });
  it('refuses by area, not by side: a long thin image is fine until its area passes the cap', () => {
    expect(planExport({ x: 0, y: 0, w: 8192, h: 2048 }, 1).ok).toBe(true); // 16,777,216 px: the cap itself
    expect(planExport({ x: 0, y: 0, w: 8192, h: 2049 }, 1)).toEqual({ ok: false, code: 'too_large' });
  });
  it('refuses a 1440 x 900 frame at 4x (20.7 M px) and takes it at 3x', () => {
    const box = { x: 0, y: 0, w: 1440, h: 900 };
    expect(planExport(box, 3)).toEqual({ ok: true, scale: 3, width: 4320, height: 2700 });
    expect(planExport(box, 4)).toEqual({ ok: false, code: 'too_large' });
  });
});

describe('exportSize', () => {
  it('is the size planExport (and so planCapture, and the canvas drawing) gives, at every scale', () => {
    const boxes = [
      { x: 0, y: 0, w: 101, h: 33 },
      { x: 0, y: 0, w: 100.34, h: 10.01 },
      { x: 0, y: 0, w: 1440, h: 900 },
      { x: 5, y: 7, w: 1, h: 1 },
      { x: 0, y: 0, w: 390.5, h: 844.25 },
    ];
    for (const box of boxes) {
      for (const scale of EXPORT_SCALES) {
        const plan = planExport(box, scale);
        const size = exportSize(box, scale);
        if (plan.ok) expect({ width: plan.width, height: plan.height }, `${box.w} x ${box.h} at ${scale}x`).toEqual(size);
        else expect(size.width * size.height).toBeGreaterThan(EXPORT_MAX_PIXELS); // refused only for area
      }
    }
    expect(exportSize({ x: 0, y: 0, w: 101, h: 33 }, 0.5)).toEqual({ width: 51, height: 17 });
    expect(exportSize({ x: 0, y: 0, w: 100.34, h: 10 }, 3)).toEqual({ width: 302, height: 30 });
    expect(exportSize({ x: 0, y: 0, w: 1440, h: 900 }, 4)).toEqual({ width: 5760, height: 3600 });
  });
});

describe('largestFittingScale', () => {
  it('is the biggest offered scale whose image fits the cap', () => {
    expect(largestFittingScale({ x: 0, y: 0, w: 600, h: 400 })).toBe(4);
    expect(largestFittingScale({ x: 0, y: 0, w: 1440, h: 900 })).toBe(3);
    expect(largestFittingScale({ x: 0, y: 0, w: 3000, h: 2000 })).toBe(1);
    expect(largestFittingScale({ x: 0, y: 0, w: 5000, h: 4000 })).toBe(0.5);
  });
  it('is null when not even 0.5x fits, or there is no node', () => {
    expect(largestFittingScale({ x: 0, y: 0, w: 9000, h: 9000 })).toBeNull();
    expect(largestFittingScale(null)).toBeNull();
  });
});
