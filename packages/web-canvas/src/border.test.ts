import { describe, expect, it } from 'vitest';
import { borderReach, borderRing, sameBorder, type SceneBorder } from './border';

const border = (o: Partial<SceneBorder> = {}): SceneBorder => ({ width: 6, color: 0xff0000, alpha: 1, position: 'inside', ...o });

describe('borderRing', () => {
  it('an inside border lies in the box: its outer edge is the box, its inner edge is the box shrunk by the width', () => {
    expect(borderRing(100, 60, 0, border())).toEqual({
      outer: { x: 0, y: 0, w: 100, h: 60, r: 0 },
      inner: { x: 6, y: 6, w: 88, h: 48, r: 0 },
    });
  });

  it('a centred border straddles the edge by half its width on each side', () => {
    expect(borderRing(100, 60, 0, border({ position: 'center' }))).toEqual({
      outer: { x: -3, y: -3, w: 106, h: 66, r: 0 },
      inner: { x: 3, y: 3, w: 94, h: 54, r: 0 },
    });
  });

  it('an outside border lies around the box: its inner edge is the box, its outer edge is the box grown by the width', () => {
    expect(borderRing(100, 60, 0, border({ position: 'outside' }))).toEqual({
      outer: { x: -6, y: -6, w: 112, h: 72, r: 0 },
      inner: { x: 0, y: 0, w: 100, h: 60, r: 0 },
    });
  });

  it('a radius follows the border as the export writes it: the outer radius grows by the share outside the box, the inner one is the outer less the width', () => {
    const at = (position: SceneBorder['position']) => borderRing(100, 60, 10, border({ position }));
    expect(at('inside')).toMatchObject({ outer: { r: 10 }, inner: { r: 4 } });
    expect(at('center')).toMatchObject({ outer: { r: 13 }, inner: { r: 7 } });
    expect(at('outside')).toMatchObject({ outer: { r: 16 }, inner: { r: 10 } });
    // A radius is held to half the smaller side, as the box is: a pill stays a pill.
    expect(borderRing(100, 40, 50, border({ position: 'inside' }))).toMatchObject({ outer: { r: 20 }, inner: { r: 14 } });
    expect(borderRing(100, 40, 50, border({ position: 'outside' }))).toMatchObject({ outer: { r: 26 }, inner: { r: 20 } });
  });

  it('the inner radius never goes below 0', () => {
    expect(borderRing(100, 60, 2, border({ width: 6 }))).toMatchObject({ outer: { r: 2 }, inner: { r: 0 } });
  });

  it('an inside border as wide as half the box or more fills the box: there is no inner edge', () => {
    expect(borderRing(100, 60, 0, border({ width: 30 })).inner).toBeNull();
    expect(borderRing(100, 60, 0, border({ width: 40 })).inner).toBeNull();
    // As in CSS, the border box is then not smaller than twice the width: the ring is taller than the box, from its top.
    expect(borderRing(100, 60, 0, border({ width: 40 })).outer).toEqual({ x: 0, y: 0, w: 100, h: 80, r: 0 });
    expect(borderRing(100, 60, 0, border({ width: 29 })).inner).not.toBeNull();
    expect(borderRing(100, 60, 0, border({ width: 70, position: 'center' })).inner).toBeNull();
  });
});

describe('borderRing of a box with no size', () => {
  it('a box with no height has a ring as high as both of its borders, from its top; an outside one is centred on the box', () => {
    expect(borderRing(200, 0, 0, border({ width: 2 }))).toEqual({ outer: { x: 0, y: 0, w: 200, h: 4, r: 0 }, inner: null });
    expect(borderRing(0, 0, 0, border({ width: 2, position: 'outside' }))).toEqual({ outer: { x: -2, y: -2, w: 4, h: 4, r: 0 }, inner: null });
  });
});

describe('sameBorder', () => {
  it('compares every member', () => {
    expect(sameBorder(border(), border())).toBe(true);
    expect(sameBorder(undefined, undefined)).toBe(true);
    expect(sameBorder(border(), undefined)).toBe(false);
    for (const o of [{ width: 7 }, { color: 1 }, { alpha: 0.5 }, { position: 'outside' as const }]) expect(sameBorder(border(), border(o))).toBe(false);
  });
});

describe('borderReach', () => {
  it('is how far the border lies outside the box: all of the width outside, half of it centred, none inside', () => {
    expect(borderReach(border({ position: 'outside' }))).toBe(6);
    expect(borderReach(border({ position: 'center' }))).toBe(3);
    expect(borderReach(border({ position: 'inside' }))).toBe(0);
    expect(borderReach(undefined)).toBe(0);
  });
});
