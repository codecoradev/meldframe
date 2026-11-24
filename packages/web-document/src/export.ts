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
 * The file name of an export: the layer's name, with the characters a file system refuses (and control characters)
 * turned into `-`, cut to 80 characters, then `@<scale>x` for every scale but 1 (`@0.5x`, `@2x`, `@3x`, `@4x`), then `.png` (or the extension asked for: the HTML
 * of a frame is `.html`, always at scale 1). A name that is empty after that
 * falls back to the node's id.
 */
export function exportFileName(name: string, id: string, scale: ExportScale, ext: 'png' | 'html' = 'png'): string {
  // Characters that reorder text are dropped first: a name cannot be made to read as another extension.
  const clean = (s: string) =>
    Array.from(s.replace(/[\u200e\u200f\u202a-\u202e\u2066-\u2069]/g, '').replace(/[\\/:*?"<>|\u0000-\u001f\u007f]/g, '-').trim())
      .slice(0, 80)
      .join('')
      .replace(/^[.\s]+/, '');
  let base = clean(name) || clean(id) || 'export';
  // Windows refuses these names whatever follows the first dot (NUL, NUL.txt, NUL.tar.gz).
  const stem = /^(con|prn|aux|nul|com[1-9]|lpt[1-9])(?=\.|$)/i.exec(base);
  if (stem) base = `${stem[0]}-${base.slice(stem[0].length)}`;
  return `${base}${scale === 1 ? '' : `@${exportScaleLabel(scale)}`}.${ext}`;
}
