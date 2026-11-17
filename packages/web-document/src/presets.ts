// Frame presets: where a new frame of a preset goes. The sizes and names are the app layer's (`FramePreset`); the sizes
// are repeated here only to centre the frame, and a test pins them to the same numbers.
import type { Action, FramePreset } from '@meldframe/protocol';

const SIZES: Record<FramePreset, { w: number; h: number }> = {
  desktop: { w: 1440, h: 900 },
  tablet: { w: 768, h: 1024 },
  mobile: { w: 390, h: 844 },
};

export const presetSize = (p: FramePreset): { w: number; h: number } => SIZES[p];

/** The action that makes a top-level frame of the preset with its centre on `centre` (world units, whole numbers). */
export function presetAction(p: FramePreset, centre: { x: number; y: number }): Action {
  const { w, h } = SIZES[p];
  return { type: 'createPreset', preset: p, x: Math.round(centre.x - w / 2), y: Math.round(centre.y - h / 2), parent: null };
}
