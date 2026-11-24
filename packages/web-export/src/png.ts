// PNG export of a frame (E4.3), the part that is not drawing: what the image would be, and saving it. The canvas draws the
// frame (a capture is drawn the same way); the page gives this module the canvas's box and drawing, and the browser's save.
import type { Rect } from '@meldframe/web-canvas/geometry';
import { type ExportScale, EXPORT_MAX_PIXELS, EXPORT_SCALES, exportFileName, exportScaleLabel, exportSize, largestFittingScale, planExport } from '@meldframe/web-document/export';

/** The frame an export acts on (`state.export.target`). */
export interface Frame {
  id: string;
  name: string;
}

/** The size of an image at a scale. */
export interface PngSize {
  scale: ExportScale;
  width: number;
  height: number;
}

/**
 * What an export would be. A refusal for size says the size it would have and the largest scale that fits (`fits`; null
 * when not even the smallest does), so the section can say it before anything is saved.
 */
export type PngPlan =
  | { ok: true; file: string; width: number; height: number }
  | { ok: false; why: 'nothing' }
  | { ok: false; why: 'too_large'; asked: PngSize; fits: PngSize | null };

/** The file and the size of the image of `frame` at `scale`, or why there is none. `box` is what the canvas draws for the frame. */
export function pngPlan(box: Rect | null, frame: Frame, scale: ExportScale): PngPlan {
  const plan = planExport(box, scale);
  if (plan.ok) return { ok: true, file: exportFileName(frame.name, frame.id, scale), width: plan.width, height: plan.height };
  if (plan.code !== 'too_large' || !box) return { ok: false, why: 'nothing' };
  const best = largestFittingScale(box);
  return { ok: false, why: 'too_large', asked: { scale, ...exportSize(box, scale) }, fits: best === null ? null : { scale: best, ...exportSize(box, best) } };
}

/** The options of a scale control, in the order of `EXPORT_SCALES`, and the scale a chosen value stands for. */
export const SCALE_OPTIONS = EXPORT_SCALES.map((s) => ({ value: String(s), label: exportScaleLabel(s) }));
export const scaleOf = (value: string): ExportScale => EXPORT_SCALES.find((s) => String(s) === value) ?? 1;

const SIDE = Math.sqrt(EXPORT_MAX_PIXELS);
const at = (s: PngSize) => `${exportScaleLabel(s.scale)} (${s.width} × ${s.height} px)`;

/** The plan in words: the file and its size in pixels, or the reason there is none (and the scale that would fit). */
export function pngText(plan: PngPlan): string {
  if (plan.ok) return `${plan.file} · ${plan.width} × ${plan.height} px`;
  if (plan.why === 'nothing') return 'There is nothing to export.';
  const first = `At ${exportScaleLabel(plan.asked.scale)} the image would be ${plan.asked.width} × ${plan.asked.height} px, more than the limit of ${SIDE} × ${SIDE} px in area.`;
  const last = EXPORT_SCALES[EXPORT_SCALES.length - 1]!;
  return plan.fits ? `${first} The largest scale that fits is ${at(plan.fits)}.` : `${first} No scale from ${exportScaleLabel(EXPORT_SCALES[0])} to ${exportScaleLabel(last)} fits this frame.`;
}

/** What the page lends: the canvas's box of a node and its offscreen drawing, the browser's save, and the toast. */
export interface PngDeps {
  box(id: string): Rect | null;
  draw(box: Rect, scale: ExportScale): Promise<Blob>;
  save(blob: Blob, fileName: string): void;
  notify(text: string): void;
}

/** Draw the frame at `scale` and save it; a refusal or a failed drawing is said and nothing is saved. */
export async function savePng(deps: PngDeps, frame: Frame, scale: ExportScale): Promise<void> {
  const box = deps.box(frame.id);
  const plan = pngPlan(box, frame, scale);
  if (!plan.ok || !box) return deps.notify(pngText(plan.ok ? { ok: false, why: 'nothing' } : plan));
  try {
    deps.save(await deps.draw(box, scale), plan.file);
  } catch {
    deps.notify('The image could not be drawn.');
  }
}
