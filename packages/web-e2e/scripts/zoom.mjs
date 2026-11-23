// The zoom readout of the editor shows a whole percent: a real zoom of 42.5% is written 43%. A browser check that divides
// a distance on the screen by the readout is therefore off by up to half a percent of the distance, and by more than a
// small tolerance once the distance is large (issue 226). The real zoom is within half a step of the readout, so the
// distance on the canvas is within the range that gives, and `withinReadout` checks against that range.

/** Half of the readout's step, as a fraction (a whole percent is 0.01). */
export const READOUT_HALF_STEP = 0.005;

const EPS = 1e-9;

/**
 * True when `actual`, a distance on the canvas, is what `pixels` on the screen come to at any zoom that the whole-percent
 * `readout` (0.43 for "43%") can stand for, give or take `tol`.
 */
export function withinReadout(actual, pixels, readout, tol) {
  const lo = pixels / (readout + READOUT_HALF_STEP);
  const hi = pixels / (readout - READOUT_HALF_STEP);
  return actual >= lo - tol - EPS && actual <= hi + tol + EPS;
}
