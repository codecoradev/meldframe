// PNG export of a frame (E4.3), the part that is not drawing: what the image would be, and saving it. The canvas draws the
// frame (a capture is drawn the same way); the page gives this module the canvas's box and drawing, and the browser's save.
import type { Rect } from '@meldframe/web-canvas/geometry';
import { type ExportScale, EXPORT_MAX_PIXELS, exportFileName, planExport } from '@meldframe/web-document/export';

/** The frame an export acts on (`state.export.target`). */
export interface Frame {
  id: string;
  name: string;
}

export type PngPlan = { ok: true; file: string; width: number; height: number } | { ok: false; why: 'nothing' | 'too_large' };

/** The file and the size of the image of `frame` at `scale`, or why there is none. `box` is what the canvas draws for the frame. */
export function pngPlan(box: Rect | null, frame: Frame, scale: ExportScale): PngPlan {
  const plan = planExport(box, scale);
  if (plan.ok) return { ok: true, file: exportFileName(frame.name, frame.id, scale), width: plan.width, height: plan.height };
  return { ok: false, why: plan.code === 'too_large' ? 'too_large' : 'nothing' };
}

const SIDE = Math.sqrt(EXPORT_MAX_PIXELS);

/** The plan in words: the file and its size in pixels, or the reason there is none. */
export function pngText(plan: PngPlan): string {
  if (plan.ok) return `${plan.file} · ${plan.width} × ${plan.height} px`;
  return plan.why === 'too_large' ? `The image would be larger than ${SIDE} x ${SIDE} pixels in area.` : 'There is nothing to export.';
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
