import { describe, expect, it } from 'vitest';
import { gradientLine, radialRadius, sameGradient } from './gradient';

const near = (a: number, b: number) => expect(a).toBeCloseTo(b, 6);

describe('gradientLine (CSS angles: 0 up, 90 right, 180 down)', () => {
  it('180 runs from the top edge to the bottom edge through the centre', () => {
    const l = gradientLine(180, 100, 50);
    [l.x0, l.y0, l.x1, l.y1].forEach((v, i) => near(v, [50, 0, 50, 50][i]!));
  });
  it('90 runs from the left edge to the right edge', () => {
    const l = gradientLine(90, 100, 50);
    [l.x0, l.y0, l.x1, l.y1].forEach((v, i) => near(v, [0, 25, 100, 25][i]!));
  });
  it('0 runs bottom to top and 270 right to left', () => {
    const up = gradientLine(0, 100, 50);
    [up.x0, up.y0, up.x1, up.y1].forEach((v, i) => near(v, [50, 50, 50, 0][i]!));
    const left = gradientLine(270, 100, 50);
    [left.x0, left.y0, left.x1, left.y1].forEach((v, i) => near(v, [100, 25, 0, 25][i]!));
  });
  it('45 on a square reaches the two opposite corners, as CSS does', () => {
    const l = gradientLine(45, 100, 100);
    [l.x0, l.y0, l.x1, l.y1].forEach((v, i) => near(v, [0, 100, 100, 0][i]!));
  });
  it('a full turn is the same line', () => {
    const a = gradientLine(30, 80, 40);
    const b = gradientLine(390, 80, 40);
    [a.x0, a.y0, a.x1, a.y1].forEach((v, i) => near(v, [b.x0, b.y0, b.x1, b.y1][i]!));
  });
});

describe('radialRadius', () => {
  it('reaches the farthest corner from the centre', () => near(radialRadius(60, 80), 50));
});

describe('sameGradient', () => {
  const g = { kind: 'linear' as const, angle: 90, stops: [{ pos: 0, color: '#fff' }, { pos: 1, color: '#000' }] };
  it('compares by value, and undefined equals undefined', () => {
    expect(sameGradient(g, { ...g, stops: g.stops.map((s) => ({ ...s })) })).toBe(true);
    expect(sameGradient(undefined, undefined)).toBe(true);
    expect(sameGradient(g, undefined)).toBe(false);
    expect(sameGradient(g, { ...g, angle: 91 })).toBe(false);
    expect(sameGradient(g, { ...g, stops: [g.stops[0]!, { pos: 1, color: '#001' }] })).toBe(false);
  });
});
