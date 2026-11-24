import { describe, expect, it } from 'vitest';
import { MAX_BLUR, blurFilterSettings, blurReach, sameShadows, shadowBounds, shadowShape, type SceneShadow } from './shadow';

const shadow = (o: Partial<SceneShadow> = {}): SceneShadow => ({ x: 0, y: 0, blur: 0, spread: 0, color: 0, alpha: 1, ...o });

describe('shadowShape', () => {
  it('is the box moved by the offset, with the radius of the box', () => {
    expect(shadowShape(100, 60, 10, shadow({ x: 5, y: -3 }))).toEqual({ x: 5, y: -3, w: 100, h: 60, r: 10 });
  });

  it('a spread grows the shape on every side and a negative one shrinks it', () => {
    expect(shadowShape(100, 60, 0, shadow({ spread: 8 }))).toEqual({ x: -8, y: -8, w: 116, h: 76, r: 0 });
    expect(shadowShape(100, 60, 0, shadow({ spread: -8, x: 2 }))).toEqual({ x: 10, y: 8, w: 84, h: 44, r: 0 });
  });

  it('there is nothing left of a shadow shrunk to no width or height', () => {
    expect(shadowShape(100, 60, 0, shadow({ spread: -30 }))).toBeNull();
    expect(shadowShape(100, 60, 0, shadow({ spread: -50 }))).toBeNull();
    expect(shadowShape(0, 0, 0, shadow())).toBeNull();
    // A box with no width still casts a shadow once a spread gives it one.
    expect(shadowShape(0, 40, 0, shadow({ spread: 3 }))).toEqual({ x: -3, y: -3, w: 6, h: 46, r: 0 });
  });

  it('there is no shape for a number that is not finite', () => {
    expect(shadowShape(NaN, 60, 0, shadow())).toBeNull();
    expect(shadowShape(100, Infinity, 0, shadow())).toBeNull();
    expect(shadowShape(100, 60, 0, shadow({ x: NaN }))).toBeNull();
    expect(shadowShape(100, 60, 0, shadow({ spread: Infinity }))).toBeNull();
  });

  it('a radius grows with a spread as the browser draws it: by all of it when the radius is the larger, less when the corner is sharper', () => {
    expect(shadowShape(100, 60, 20, shadow({ spread: 10 }))?.r).toBe(30);
    expect(shadowShape(100, 60, 30, shadow({ spread: 5 }))?.r).toBe(35);
    // The ratio of the CSS rule: 1 + (10 / 20 - 1)^3 = 0.875, so 10 + 20 * 0.875 and not 10 + 20.
    expect(shadowShape(100, 60, 10, shadow({ spread: 20 }))?.r).toBeCloseTo(27.5, 10);
    // A square corner stays square, whatever the spread.
    expect(shadowShape(100, 60, 0, shadow({ spread: 20 }))?.r).toBe(0);
  });

  it('a negative spread takes the radius off, not below 0', () => {
    expect(shadowShape(100, 60, 20, shadow({ spread: -5 }))?.r).toBe(15);
    expect(shadowShape(100, 60, 10, shadow({ spread: -15 }))?.r).toBe(0);
  });

  it('the radius is held to half of the smaller side, of the box and of the shadow', () => {
    expect(shadowShape(100, 60, 500, shadow())?.r).toBe(30);
    expect(shadowShape(100, 60, 500, shadow({ spread: -10 }))?.r).toBe(20);
    expect(shadowShape(100, 60, 30, shadow({ spread: 40 }))?.r).toBeLessThanOrEqual(70);
    expect(shadowShape(100, 60, -4, shadow())?.r).toBe(0);
  });
});

describe('sameShadows', () => {
  it('none and an empty list are the same, and a list is the same when every number is', () => {
    expect(sameShadows(undefined, undefined)).toBe(true);
    expect(sameShadows(undefined, [])).toBe(true);
    expect(sameShadows([shadow()], [shadow()])).toBe(true);
    expect(sameShadows([shadow()], undefined)).toBe(false);
  });

  it('differs in any field, in the order and in the length', () => {
    for (const k of ['x', 'y', 'blur', 'spread', 'color', 'alpha'] as const) expect(sameShadows([shadow()], [shadow({ [k]: 0.5 })]), k).toBe(false);
    const a = shadow({ x: 1 });
    const b = shadow({ x: 2 });
    expect(sameShadows([a, b], [b, a])).toBe(false);
    expect(sameShadows([a], [a, b])).toBe(false);
  });
});

describe('shadowBounds', () => {
  const box = { x: 100, y: 50, w: 40, h: 20 };

  it('is the rectangle all the shadows cover, blur included, or null for none', () => {
    expect(shadowBounds(box, undefined)).toBeNull();
    expect(shadowBounds(box, [])).toBeNull();
    expect(shadowBounds(box, [shadow({ y: 4, blur: 8, spread: 2 })])).toEqual({ x: 90, y: 44, w: 60, h: 40 });
    expect(shadowBounds(box, [shadow({ x: 10 }), shadow({ x: -10, y: 5, blur: 3 })])).toEqual({ x: 87, y: 50, w: 63, h: 28 });
  });

  it('leaves out a shadow with nothing left of it and takes the blur as its reach', () => {
    expect(shadowBounds(box, [shadow({ spread: -30, blur: 20 })])).toBeNull();
    expect(blurReach(shadow({ blur: 12 }))).toBe(12);
  });
});

describe('shadows that paint nothing and blurs that are too big', () => {
  const box = { x: 100, y: 50, w: 40, h: 20 };

  it('a shadow with an alpha of 0 paints nothing, so it has no reach', () => {
    expect(shadowBounds(box, [shadow({ blur: 30, spread: 5, alpha: 0 })])).toBeNull();
    expect(shadowBounds(box, [shadow({ alpha: 0, x: 500 }), shadow({ y: 4, blur: 2 })])).toEqual({ x: 98, y: 52, w: 44, h: 24 });
  });

  it('the reach of a blur is 0 for a blur below 0 or one that is not a number', () => {
    expect(blurReach(shadow({ blur: -5 }))).toBe(0);
    expect(blurReach(shadow({ blur: NaN }))).toBe(0);
    expect(blurReach(shadow({ blur: Infinity }))).toBe(0);
    expect(shadowBounds(box, [shadow({ blur: NaN, x: 3 })])).toEqual({ x: 103, y: 50, w: 40, h: 20 });
  });

  it('an offset and a spread count for at most MAX_BLUR in the reach, as a blur does: a far shadow does not make a huge capture', () => {
    // Capped to 1024: the shape is the box moved by (1024, -1024) and grown by 1024; only the reach is capped, not the drawing.
    expect(shadowBounds(box, [shadow({ x: 1e6, y: -1e6, spread: 1e6 })])).toEqual({ x: 100, y: 50 - 2048, w: 40 + 2048, h: 20 + 2048 });
    expect(shadowBounds(box, [shadow({ x: -1e6 })])).toEqual({ x: 100 - MAX_BLUR, y: 50, w: 40, h: 20 });
    expect(shadowShape(40, 20, 0, shadow({ x: 1e6 }))).toMatchObject({ x: 1e6 });
    // A spread that shrinks it to nothing still leaves nothing.
    expect(shadowBounds(box, [shadow({ spread: -1e6 })])).toBeNull();
  });

  it('the reach of a blur stops at MAX_BLUR, so a capture of a shadow with a huge blur is not a huge image', () => {
    expect(blurReach(shadow({ blur: 100 }))).toBe(100);
    expect(blurReach(shadow({ blur: 100000 }))).toBe(MAX_BLUR);
    expect(shadowBounds(box, [shadow({ blur: 100000 })])).toEqual({ x: 100 - MAX_BLUR, y: 50 - MAX_BLUR, w: 40 + 2 * MAX_BLUR, h: 20 + 2 * MAX_BLUR });
  });
});

describe('blurFilterSettings', () => {
  it('is a strength of half the blur on the screen, and room for 1.5 times the blur and a little more', () => {
    expect(blurFilterSettings(12, 1)).toEqual({ strength: 6, padding: 20 });
    expect(blurFilterSettings(12, 2)).toEqual({ strength: 12, padding: 38 });
    expect(blurFilterSettings(16, 0.25)).toEqual({ strength: 2, padding: 8 });
  });

  it('never asks the renderer for more than MAX_BLUR pixels of blur: a blur of 5000 at a zoom of 2 does not make a padding of 15000', () => {
    const s = blurFilterSettings(5000, 2);
    expect(s.strength).toBe(MAX_BLUR / 2);
    expect(s.padding).toBe(Math.ceil(1.5 * MAX_BLUR) + 2);
    expect(blurFilterSettings(100, 1000).padding).toBe(s.padding);
    expect(blurFilterSettings(0, 2)).toEqual({ strength: 0, padding: 2 });
  });
});
