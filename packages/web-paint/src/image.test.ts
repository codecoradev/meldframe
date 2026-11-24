import { describe, expect, it } from 'vitest';
import { fitRect, sameImage } from './image';

describe('fitRect', () => {
  it('fill stretches the image to the box whatever its shape', () => {
    expect(fitRect('fill', 200, 100, 50, 50)).toEqual({ x: 0, y: 0, w: 200, h: 100 });
  });

  it('cover scales to cover the box and centres what sticks out', () => {
    // A square image in a wide box: scaled to the width, the height overflows equally above and below.
    expect(fitRect('cover', 200, 100, 50, 50)).toEqual({ x: 0, y: -50, w: 200, h: 200 });
    // A wide image in a tall box: scaled to the height, the width overflows equally left and right.
    expect(fitRect('cover', 100, 200, 100, 50)).toEqual({ x: -150, y: 0, w: 400, h: 200 });
  });

  it('contain scales to fit inside the box and centres it', () => {
    expect(fitRect('contain', 200, 100, 50, 50)).toEqual({ x: 50, y: 0, w: 100, h: 100 });
    expect(fitRect('contain', 100, 200, 100, 50)).toEqual({ x: 0, y: 75, w: 100, h: 50 });
  });

  it('an image of the box shape fills it for every fit', () => {
    for (const fit of ['cover', 'contain', 'fill'] as const) {
      expect(fitRect(fit, 120, 60, 240, 120)).toEqual({ x: 0, y: 0, w: 120, h: 60 });
    }
  });

  it('an image with no size draws into the box instead of dividing by zero', () => {
    expect(fitRect('cover', 80, 40, 0, 0)).toEqual({ x: 0, y: 0, w: 80, h: 40 });
  });
});

describe('sameImage', () => {
  const a = { assetId: 'a'.repeat(64), fit: 'cover' as const };
  it('compares the asset and the fit', () => {
    expect(sameImage(a, { ...a })).toBe(true);
    expect(sameImage(a, { ...a, fit: 'fill' })).toBe(false);
    expect(sameImage(a, { ...a, assetId: 'b'.repeat(64) })).toBe(false);
    expect(sameImage(a, undefined)).toBe(false);
    expect(sameImage(undefined, undefined)).toBe(true);
  });
});
