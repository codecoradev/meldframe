import { describe, expect, it } from 'vitest';
import { withinReadout } from './zoom.mjs';

// The zoom readout shows a whole percent: 42.5% is written 43%. A check that divides the pointer's distance by the
// readout is off by up to half a percent of the distance, which is more than a small tolerance at a larger distance.
describe('a distance on the canvas, worked out from the rounded zoom readout', () => {
  it('accepts what the app wrote when the real zoom is 42.5% and the readout says 43% (issue 226)', () => {
    // The pointer moved 80 px; 80 / 0.425 = 188.235 is the right answer, and 80 / 0.43 = 186.05 is the readout's.
    expect(withinReadout(188.23529411764707, 80, 0.43, 2)).toBe(true);
    expect(withinReadout(186.04651162790697, 80, 0.43, 2)).toBe(true);
  });
  it('accepts every real zoom that the readout rounds to', () => {
    for (const real of [0.425, 0.4251, 0.43, 0.4349]) expect(withinReadout(80 / real, 80, 0.43, 0), `${real}`).toBe(true);
  });
  it('still refuses a value that is wrong by more than the readout and the tolerance explain', () => {
    expect(withinReadout(80 / 0.5, 80, 0.43, 2)).toBe(false);
    expect(withinReadout(80, 80, 0.43, 2)).toBe(false);
    expect(withinReadout(190.5, 80, 0.43, 2)).toBe(false);
  });
  it('applies the tolerance on both sides of the range', () => {
    expect(withinReadout(183.9 - 1.9, 80, 0.43, 2)).toBe(true);
    expect(withinReadout(183.9 - 2.5, 80, 0.43, 2)).toBe(false);
    expect(withinReadout(188.24 + 1.9, 80, 0.43, 2)).toBe(true);
    expect(withinReadout(188.24 + 2.5, 80, 0.43, 2)).toBe(false);
  });
  it('has its edges where the readout rounds: just outside the zoom range is refused with no tolerance', () => {
    // A readout of 43% stands for a zoom from 0.425 to 0.435; 80 / 0.4351 and 80 / 0.4249 lie just outside it.
    expect(withinReadout(80 / 0.4351, 80, 0.43, 0)).toBe(false);
    expect(withinReadout(80 / 0.4249, 80, 0.43, 0)).toBe(false);
    expect(withinReadout(80 / 0.4349, 80, 0.43, 0)).toBe(true);
    expect(withinReadout(80 / 0.4251, 80, 0.43, 0)).toBe(true);
  });
  it('holds at the smallest readout, where half a percent is a quarter of the zoom', () => {
    expect(withinReadout(80 / 0.02, 80, 0.02, 0)).toBe(true);
    expect(withinReadout(80 / 0.0249, 80, 0.02, 0)).toBe(true);
    expect(withinReadout(80 / 0.0151, 80, 0.02, 0)).toBe(true);
    expect(withinReadout(80 / 0.0149, 80, 0.02, 0)).toBe(false);
  });
});
