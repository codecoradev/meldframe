import { describe, expect, it } from 'vitest';
import { sameCamera, shouldRefit, type RefitFacts } from './fit';

const cam = { x: 10, y: 20, zoom: 0.5 };
const facts = (over: Partial<RefitFacts> = {}): RefitFacts => ({ fitted: true, refitted: false, setsAtFit: 1, sets: 2, fitCamera: cam, camera: { ...cam }, ...over });

describe('shouldRefit', () => {
  it('fits again once the measurer was given again after the first fit, with the camera where the fit left it', () => {
    expect(shouldRefit(facts())).toBe(true);
  });
  it('does not before the first fit, or when the measurer has not changed since it (the fonts were there already)', () => {
    expect(shouldRefit(facts({ fitted: false, fitCamera: null }))).toBe(false);
    expect(shouldRefit(facts({ sets: 1 }))).toBe(false);
  });
  it('does it once: a second change of the measurer (another face loading) leaves the camera alone', () => {
    expect(shouldRefit(facts({ refitted: true, sets: 3 }))).toBe(false);
  });
  it('does not when the person has zoomed or panned since the fit: the camera is theirs', () => {
    for (const camera of [{ ...cam, zoom: 1 }, { ...cam, x: 11 }, { ...cam, y: 0 }]) expect(shouldRefit(facts({ camera }))).toBe(false);
  });
  it('compares cameras by their three numbers', () => {
    expect(sameCamera(cam, { ...cam })).toBe(true);
    expect(sameCamera(cam, { ...cam, zoom: 0.50001 })).toBe(false);
  });
});
