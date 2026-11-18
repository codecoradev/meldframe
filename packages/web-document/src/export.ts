// PNG export from the editor (E4.3): a node is drawn offscreen at 1x or 2x and saved as a file. The drawing is the one
// a capture uses (`planCapture`, the canvas's `capture`); this module only decides the scales, the pixel limit and the
// file name.
import type { Rect } from '@meldframe/web-canvas/geometry';
import { type CapturePlan, planCapture } from './capture';

/** The scales the editor offers. */
export const EXPORT_SCALES = [1, 2] as const;
export type ExportScale = (typeof EXPORT_SCALES)[number];

/** The most pixels (width times height) of an exported image: the area limit of a capture, 4096 x 4096. */
export const EXPORT_MAX_PIXELS = 4096 * 4096;

/** Whether the node fits the limit at `scale`, and the size of the image. */
export function planExport(box: Rect | null, scale: ExportScale): CapturePlan {
  return planCapture({ scale, maxPixels: EXPORT_MAX_PIXELS }, box);
}

/**
 * The file name of an export: the layer's name, with the characters a file system refuses (and control characters)
 * turned into `-`, cut to 80 characters, then `@2x` for a scale above 1, then `.png`. A name that is empty after that
 * falls back to the node's id.
 */
export function exportFileName(name: string, id: string, scale: ExportScale): string {
  const clean = (s: string) =>
    Array.from(s.replace(/[\\/:*?"<>|\u0000-\u001f\u007f]/g, '-').trim())
      .slice(0, 80)
      .join('')
      .replace(/^[.\s]+/, '');
  let base = clean(name) || clean(id) || 'export';
  // Windows refuses these names whatever follows the first dot (NUL, NUL.txt, NUL.tar.gz).
  const stem = /^(con|prn|aux|nul|com[1-9]|lpt[1-9])(?=\.|$)/i.exec(base);
  if (stem) base = `${stem[0]}-${base.slice(stem[0].length)}`;
  return `${base}${scale > 1 ? `@${scale}x` : ''}.png`;
}
