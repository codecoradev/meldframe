// Frame presets: the rows of the New frame menu and where a new frame of a preset goes. Pure. The preset ids, sizes and
// names are the app layer's (`FramePreset` in `meldframe-app-model`, which makes the frame); this table repeats the sizes
// to centre the frame and to label the rows, and a test pins both to the app layer's table. To change the list, edit
// `FramePreset` first (type-checked here: a preset missing from `PRESETS` does not compile), then this file.
import type { Action, FramePreset } from '@meldframe/protocol';

export const PRESETS: Record<FramePreset, { name: string; w: number; h: number }> = {
  desktop: { name: 'Desktop', w: 1440, h: 900 },
  desktopHd: { name: 'Desktop HD', w: 1920, h: 1080 },
  tablet: { name: 'Tablet', w: 768, h: 1024 },
  mobile: { name: 'Mobile', w: 390, h: 844 },
  phoneSmall: { name: 'Phone small', w: 375, h: 667 },
  a4: { name: 'A4', w: 595, h: 842 },
  letter: { name: 'Letter', w: 612, h: 792 },
};

/** The presets in the order the menu lists them. */
export const PRESET_ORDER: readonly FramePreset[] = ['desktop', 'desktopHd', 'tablet', 'mobile', 'phoneSmall', 'a4', 'letter'];

export const presetSize = (p: FramePreset): { w: number; h: number } => ({ w: PRESETS[p].w, h: PRESETS[p].h });

/** The action that makes a top-level frame of the preset with its centre on `centre` (world units, whole numbers). */
export function presetAction(p: FramePreset, centre: { x: number; y: number }): Action {
  const { w, h } = PRESETS[p];
  return { type: 'createPreset', preset: p, x: Math.round(centre.x - w / 2), y: Math.round(centre.y - h / 2), parent: null };
}

/** A row of the New frame menu; `shortcut` is the slot of the menu for the text after the label, here the size. */
export interface PresetItem {
  id: FramePreset;
  label: string;
  shortcut: string;
  onSelect: () => void;
  testId: string;
}

/** The size as a row writes it: `1440 × 900`. */
export const sizeText = (p: FramePreset): string => `${PRESETS[p].w} × ${PRESETS[p].h}`;

export function presetItems(onPreset: (p: FramePreset) => void): PresetItem[] {
  return PRESET_ORDER.map((id) => ({ id, label: PRESETS[id].name, shortcut: sizeText(id), onSelect: () => onPreset(id), testId: `preset-${id}` }));
}
