import { describe, expect, it } from 'vitest';
import { fonts } from '@meldframe/web-tokens/tokens';
import { drawStyle, pixiTextStyle } from './pixitext';

describe('pixiTextStyle', () => {
  it('draws with the font stack of the page, the size, the weight as the string Pixi takes, and the wrap of the shape', () => {
    expect(pixiTextStyle({ fontSize: 20, fontWeight: 700, wrapWidth: 120 })).toEqual({
      fontFamily: fonts.sans, fontSize: 20, fontWeight: '700', wordWrap: true, wordWrapWidth: 120,
    });
  });
  it('one line is no wrap at all', () => {
    expect(pixiTextStyle({ fontSize: 12, fontWeight: 400, wrapWidth: null })).toMatchObject({ wordWrap: false });
  });
});

describe('drawStyle', () => {
  const node = { fontSize: 18, fontWeight: 600, textAlign: 'center', w: 140, fill: 0x112233 } as const;
  it('is the style the measurer measures with, plus the fill and the alignment', () => {
    expect(drawStyle(node)).toEqual({
      fontFamily: fonts.sans, fontSize: 18, fontWeight: '600', wordWrap: true, wordWrapWidth: 140, fill: 0x112233, align: 'center',
    });
  });
  it('draws a text with no fill in black, and one with no width on one line', () => {
    expect(drawStyle({ ...node, fill: null, w: 0 })).toMatchObject({ fill: 0, wordWrap: false });
  });
});
