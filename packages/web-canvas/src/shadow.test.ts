import { describe, expect, it } from 'vitest';
import { blurReach, sameShadows, shadowBounds, shadowShape, type SceneShadow } from './shadow';

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
