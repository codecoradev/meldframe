// The capture of a node as a PNG for an agent: what size the image gets and what the upload carries. Pure; the drawing
// is the canvas's, the request and the upload are the editor page's.
import type { Rect } from '@meldframe/web-canvas/geometry';

export type CapturePlan = { ok: true; scale: number; width: number; height: number } | { ok: false; code: 'node_not_found' | 'too_large' };

/** The size of the image for a box (the node, or the whole page) at the asked scale, or the code that refuses it. */
export function planCapture(req: { scale: number; maxPixels: number }, box: Rect | null): CapturePlan {
  if (!box || !(box.w > 0) || !(box.h > 0)) return { ok: false, code: 'node_not_found' };
  if (!Number.isFinite(req.scale) || req.scale <= 0) return { ok: false, code: 'too_large' };
  const width = Math.ceil(box.w * req.scale);
  const height = Math.ceil(box.h * req.scale);
  if (width * height > req.maxPixels) return { ok: false, code: 'too_large' };
  return { ok: true, scale: req.scale, width, height };
}

/** The headers of the upload (`POST /captures/{requestId}`); the login is added by whoever sends it. */
export function captureHeaders(req: { uploadToken: string }, seq: number): Record<string, string> {
  return { 'x-capture-token': req.uploadToken, 'x-capture-seq': String(seq), 'content-type': 'image/png' };
}
