import { describe, expect, it, vi } from 'vitest';
import { pngPlan, pngText, savePng, type PngDeps } from './png';

const frame = { id: 'f1', name: 'Landing Desktop' };
const box = { x: 0, y: 0, w: 1440, h: 2200 };

describe('the plan of a PNG', () => {
  it('names the file and gives the pixels at each scale', () => {
    expect(pngPlan(box, frame, 1)).toEqual({ ok: true, file: 'Landing Desktop.png', width: 1440, height: 2200 });
    expect(pngPlan(box, frame, 2)).toEqual({ ok: true, file: 'Landing Desktop@2x.png', width: 2880, height: 4400 });
    // 6000 x 4000 is over 4096 x 4096 in area; 3000 x 2000 at 1x is the largest scale that fits
    expect(pngPlan({ x: 0, y: 0, w: 3000, h: 2000 }, frame, 2)).toEqual({ ok: false, why: 'too_large', asked: { scale: 2, width: 6000, height: 4000 }, fits: { scale: 1, width: 3000, height: 2000 } });
    expect(pngPlan({ x: 0, y: 0, w: 390, h: 844 }, { id: 'm', name: 'Mobile' }, 2)).toEqual({ ok: true, file: 'Mobile@2x.png', width: 780, height: 1688 });
  });
  it('names the file and sizes of the new scales: @0.5x, @3x, @4x', () => {
    const w = { x: 0, y: 0, w: 600, h: 400 };
    expect(pngPlan(w, frame, 0.5)).toEqual({ ok: true, file: 'Landing Desktop@0.5x.png', width: 300, height: 200 });
    expect(pngPlan(w, frame, 3)).toEqual({ ok: true, file: 'Landing Desktop@3x.png', width: 1800, height: 1200 });
    expect(pngPlan(w, frame, 4)).toEqual({ ok: true, file: 'Landing Desktop@4x.png', width: 2400, height: 1600 });
  });
  it('refuses a 1440 x 900 frame at 4x and says 3x is the largest that fits', () => {
    const desk = { x: 0, y: 0, w: 1440, h: 900 };
    expect(pngPlan(desk, frame, 3).ok).toBe(true);
    expect(pngPlan(desk, frame, 4)).toEqual({ ok: false, why: 'too_large', asked: { scale: 4, width: 5760, height: 3600 }, fits: { scale: 3, width: 4320, height: 2700 } });
  });
  it('says when no scale fits', () => {
    expect(pngPlan({ x: 0, y: 0, w: 9000, h: 9000 }, frame, 0.5)).toEqual({ ok: false, why: 'too_large', asked: { scale: 0.5, width: 4500, height: 4500 }, fits: null });
  });
  it('has nothing to export without a box', () => {
    expect(pngPlan(null, frame, 1)).toEqual({ ok: false, why: 'nothing' });
  });
  it('says it in words: the file and its size, or why not', () => {
    expect(pngText(pngPlan(box, frame, 1))).toBe('Landing Desktop.png · 1440 × 2200 px');
    expect(pngText(pngPlan({ x: 0, y: 0, w: 1440, h: 900 }, { id: 'd', name: 'Desk' }, 4))).toBe(
      'At 4x the image would be 5760 × 3600 px, more than the limit of 4096 × 4096 px in area. The largest scale that fits is 3x (4320 × 2700 px).',
    );
    expect(pngText(pngPlan({ x: 0, y: 0, w: 9000, h: 9000 }, frame, 1))).toBe(
      'At 1x the image would be 9000 × 9000 px, more than the limit of 4096 × 4096 px in area. No scale from 0.5x to 4x fits this frame.',
    );
    expect(pngText(pngPlan({ x: 0, y: 0, w: 600, h: 400 }, { id: 'h', name: 'Half' }, 0.5))).toBe('Half@0.5x.png · 300 × 200 px');
    expect(pngText(pngPlan(null, frame, 1))).toBe('There is nothing to export.');
  });
});

function deps(over: Partial<PngDeps> = {}): PngDeps & { saved: [Blob, string][]; said: string[] } {
  const saved: [Blob, string][] = [];
  const said: string[] = [];
  return {
    box: () => box,
    draw: async () => new Blob(['png']),
    save: (b, n) => void saved.push([b, n]),
    notify: (t) => void said.push(t),
    ...over,
    saved,
    said,
  };
}

describe('saving a PNG', () => {
  it('draws the frame at the scale and saves it under its file name', async () => {
    const draw = vi.fn(async () => new Blob(['png']));
    const d = deps({ box: () => ({ x: 0, y: 0, w: 390, h: 844 }), draw });
    await savePng(d, { id: 'm', name: 'Mobile' }, 2);
    expect(draw).toHaveBeenCalledWith({ x: 0, y: 0, w: 390, h: 844 }, 2);
    expect(d.saved.map(([, n]) => n)).toEqual(['Mobile@2x.png']);
    expect(d.said).toEqual([]);
  });
  it('saves a half-size image under @0.5x', async () => {
    const d = deps({ box: () => ({ x: 0, y: 0, w: 390, h: 844 }) });
    await savePng(d, { id: 'm', name: 'Mobile' }, 0.5);
    expect(d.saved.map(([, n]) => n)).toEqual(['Mobile@0.5x.png']);
  });
  it('says why when there is nothing to draw or the image is too large, and saves nothing', async () => {
    const none = deps({ box: () => null });
    await savePng(none, frame, 1);
    expect(none.said).toEqual(['There is nothing to export.']);
    const big = deps({ box: () => ({ x: 0, y: 0, w: 3000, h: 2000 }) });
    await savePng(big, frame, 2);
    expect(big.said).toEqual(['At 2x the image would be 6000 × 4000 px, more than the limit of 4096 × 4096 px in area. The largest scale that fits is 1x (3000 × 2000 px).']);
    expect(none.saved.length + big.saved.length).toBe(0);
  });
  it('says so when the drawing fails, and saves nothing', async () => {
    const d = deps({ draw: async () => { throw new Error('gl lost'); } });
    await savePng(d, frame, 1);
    expect(d.said).toEqual(['The image could not be drawn.']);
    expect(d.saved).toEqual([]);
  });
});
