// PNG export from the editor (E4.3): a node is drawn offscreen at 0.5x to 4x and saved as a file. The drawing is the one
// a capture uses (`planCapture`, the canvas's `capture`); this module only decides the scales, the pixel limit, the
// rounding of the size and the file name.
import type { Rect } from '@meldframe/web-canvas/geometry';
import { type CapturePlan, planCapture } from './capture';

/** The scales the editor offers. */
export const EXPORT_SCALES = [0.5, 1, 2, 3, 4] as const;
export type ExportScale = (typeof EXPORT_SCALES)[number];

/** The most pixels (width times height) of an exported image: the area limit of a capture, 4096 x 4096. */
export const EXPORT_MAX_PIXELS = 4096 * 4096;

/** A scale as the editor words it: `0.5x`, `1x`, `3x`. */
export const exportScaleLabel = (scale: ExportScale): string => `${scale}x`;

/**
 * Whether the node fits the limit at `scale`, and the size of the image. Each side is the node's size times the scale,
 * rounded up to a whole pixel (the drawing rounds the same way, so no edge is cut and an image is never 0 px).
 */
export function planExport(box: Rect | null, scale: ExportScale): CapturePlan {
  return planCapture({ scale, maxPixels: EXPORT_MAX_PIXELS }, box);
}

/** The size in pixels of the image of a box at `scale`, whether or not it fits the limit (the sides rounded up, as `planExport` does). */
export function exportSize(box: Rect, scale: ExportScale): { width: number; height: number } {
  return { width: Math.ceil(box.w * scale), height: Math.ceil(box.h * scale) };
}

/** The largest offered scale at which the node fits the limit; null when not even the smallest does, or there is no node. */
export function largestFittingScale(box: Rect | null): ExportScale | null {
  for (let i = EXPORT_SCALES.length - 1; i >= 0; i--) {
    const scale = EXPORT_SCALES[i]!;
    if (planExport(box, scale).ok) return scale;
  }
  return null;
}

/**
 * The file an export is saved as: `stem` is the name the app layer worked out (`ExportTarget.fileStem`, safe to use as it
 * is: no hidden character, no character a file system refuses, at most 80 characters), then `@<scale>x` for every scale
 * but 1 (`@0.5x`, `@2x`, `@3x`, `@4x`), then `.png` (or the extension asked for). The page cleans nothing here.
 */
export function exportFileName(stem: string, scale: ExportScale, ext: 'png' | 'html' = 'png'): string {
  return `${stem}${scale === 1 ? '' : `@${exportScaleLabel(scale)}`}.${ext}`;
}
