import { describe, expect, it, vi } from 'vitest';
import { pngPlan, pngText, savePng, type PngDeps } from './png';

const frame = { id: 'f1', name: 'Landing Desktop' };
const box = { x: 0, y: 0, w: 1440, h: 2200 };

describe('the plan of a PNG', () => {
  it('names the file and gives the pixels at each scale', () => {
    expect(pngPlan(box, frame, 1)).toEqual({ ok: true, file: 'Landing Desktop.png', width: 1440, height: 2200 });
    expect(pngPlan(box, frame, 2)).toEqual({ ok: true, file: 'Landing Desktop@2x.png', width: 2880, height: 4400 });
    expect(pngPlan({ x: 0, y: 0, w: 3000, h: 2000 }, frame, 2)).toEqual({ ok: false, why: 'too_large' }); // 6000 x 4000 is over 4096 x 4096 in area
    expect(pngPlan({ x: 0, y: 0, w: 390, h: 844 }, { id: 'm', name: 'Mobile' }, 2)).toEqual({ ok: true, file: 'Mobile@2x.png', width: 780, height: 1688 });
  });
  it('has nothing to export without a box', () => {
    expect(pngPlan(null, frame, 1)).toEqual({ ok: false, why: 'nothing' });
  });
  it('says it in words: the file and its size, or why not', () => {
    expect(pngText(pngPlan(box, frame, 1))).toBe('Landing Desktop.png · 1440 × 2200 px');
    expect(pngText(pngPlan({ x: 0, y: 0, w: 3000, h: 2000 }, frame, 2))).toBe('The image would be larger than 4096 x 4096 pixels in area.');
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
  it('says why when there is nothing to draw or the image is too large, and saves nothing', async () => {
    const none = deps({ box: () => null });
    await savePng(none, frame, 1);
    expect(none.said).toEqual(['There is nothing to export.']);
    const big = deps({ box: () => ({ x: 0, y: 0, w: 3000, h: 2000 }) });
    await savePng(big, frame, 2);
    expect(big.said).toEqual(['The image would be larger than 4096 x 4096 pixels in area.']);
    expect(none.saved.length + big.saved.length).toBe(0);
  });
  it('says so when the drawing fails, and saves nothing', async () => {
    const d = deps({ draw: async () => { throw new Error('gl lost'); } });
    await savePng(d, frame, 1);
    expect(d.said).toEqual(['The image could not be drawn.']);
    expect(d.saved).toEqual([]);
  });
});
