import { describe, expect, it } from 'vitest';
import { presetAction, presetSize } from './presets';

describe('presetAction', () => {
  it('centres the new frame on the point and makes it a top-level frame', () => {
    expect(presetAction('mobile', { x: 1000, y: 500 })).toEqual({ type: 'createPreset', preset: 'mobile', x: 805, y: 78, parent: null });
    expect(presetAction('desktop', { x: 0, y: 0 })).toEqual({ type: 'createPreset', preset: 'desktop', x: -720, y: -450, parent: null });
  });
  it('rounds to whole units so the frame sits on the pixel grid', () => {
    const a = presetAction('tablet', { x: 100.4, y: 99.6 });
    expect(Number.isInteger((a as { x: number }).x) && Number.isInteger((a as { y: number }).y)).toBe(true);
  });
});

describe('presetSize', () => {
  it('matches the sizes of the app layer', () => {
    expect(presetSize('desktop')).toEqual({ w: 1440, h: 900 });
    expect(presetSize('tablet')).toEqual({ w: 768, h: 1024 });
    expect(presetSize('mobile')).toEqual({ w: 390, h: 844 });
  });
});
