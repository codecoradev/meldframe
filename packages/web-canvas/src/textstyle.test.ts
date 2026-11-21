import { describe, expect, it } from 'vitest';
import { alignOffset, textDrawShape, textMeasureShape } from './textstyle';

describe('the shape of a text, as drawn and as measured', () => {
  it('draws a text with the size and weight it has and wraps it at the width of its box', () => {
    expect(textDrawShape({ fontSize: 20, fontWeight: 700, w: 120.5 })).toEqual({ fontSize: 20, fontWeight: 700, wrapWidth: 120.5 });
  });
  it('does not wrap a text that has no width, and never draws below 1 px', () => {
    expect(textDrawShape({ fontSize: 0, fontWeight: 400, w: 0 })).toEqual({ fontSize: 1, fontWeight: 400, wrapWidth: null });
    expect(textDrawShape({ fontSize: 12, fontWeight: 400, w: 0.2 }).wrapWidth).toBe(1);
  });
  it('measures with the shape it will be drawn with: the same size, weight and wrap, null for one line', () => {
    expect(textMeasureShape(0.4, 700, null)).toEqual(textDrawShape({ fontSize: 0.4, fontWeight: 700, w: 0 }));
    expect(textMeasureShape(16, 500, 90)).toEqual(textDrawShape({ fontSize: 16, fontWeight: 500, w: 90 }));
    expect(textMeasureShape(16, 500, 0.3)).toEqual(textDrawShape({ fontSize: 16, fontWeight: 500, w: 0.3 }));
    expect(textMeasureShape(16, 500, 0.3).wrapWidth).toBe(1);
  });
  it('measures and draws a width that is 0, negative or not a number the same way: one line, no wrapping', () => {
    for (const w of [0, -0, -5, NaN]) {
      expect(textMeasureShape(16, 500, w), String(w)).toEqual(textDrawShape({ fontSize: 16, fontWeight: 500, w }));
      expect(textMeasureShape(16, 500, w).wrapWidth, String(w)).toBeNull();
    }  });
});

describe('alignOffset', () => {
  it('puts the block of lines at the left, the middle or the right of the box', () => {
    expect(alignOffset('left', 200, 80)).toBe(0);
    expect(alignOffset('center', 200, 80)).toBe(60);
    expect(alignOffset('right', 200, 80)).toBe(120);
  });
  it('a block wider than its box starts at the left edge, as the lines of an overflowing line do in CSS', () => {
    for (const a of ['left', 'center', 'right'] as const) expect(alignOffset(a, 50, 80)).toBe(0);
  });
});
