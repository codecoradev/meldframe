import { describe, expect, it } from 'vitest';
import { captureHeaders, planCapture } from './capture';

describe('planCapture', () => {
  it('sizes the image from the box and the scale, rounding up so no edge is cut', () => {
    expect(planCapture({ scale: 2, maxPixels: 1e9 }, { x: 5, y: 5, w: 100.2, h: 50 })).toEqual({ ok: true, scale: 2, width: 201, height: 100 });
  });
  it('refuses a node that is not there (no box) or has no area', () => {
    expect(planCapture({ scale: 1, maxPixels: 1e9 }, null)).toEqual({ ok: false, code: 'node_not_found' });
    expect(planCapture({ scale: 1, maxPixels: 1e9 }, { x: 0, y: 0, w: 0, h: 10 })).toEqual({ ok: false, code: 'node_not_found' });
  });
  it('refuses an image over the pixel limit instead of shrinking it behind the agent\'s back', () => {
    expect(planCapture({ scale: 4, maxPixels: 1000 }, { x: 0, y: 0, w: 100, h: 100 })).toEqual({ ok: false, code: 'too_large' });
    expect(planCapture({ scale: 1, maxPixels: 10000 }, { x: 0, y: 0, w: 100, h: 100 })).toMatchObject({ ok: true });
  });
  it('refuses a scale that is not a positive finite number', () => {
    for (const scale of [0, -1, Number.NaN, Number.POSITIVE_INFINITY]) {
      expect(planCapture({ scale, maxPixels: 1e9 }, { x: 0, y: 0, w: 10, h: 10 }), String(scale)).toEqual({ ok: false, code: 'too_large' });
    }
  });
});

describe('captureHeaders', () => {
  it('carries the upload token and the sequence number the image was rendered at', () => {
    expect(captureHeaders({ uploadToken: 'tok' }, 12)).toEqual({ 'x-capture-token': 'tok', 'x-capture-seq': '12', 'content-type': 'image/png' });
  });
});
