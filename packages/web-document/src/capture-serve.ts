// The editor's side of a capture request: where the node is, wait until the file is as new as the agent asked,
// draw it, upload it. Every request ends in an upload or in `captureFailed`, never silence (the agent waits for the
// answer until the server's timeout). The pieces that need the canvas and the network come in as `CaptureDeps`.
import type { Rect } from '@meldframe/web-canvas/geometry';
import { captureHeaders, planCapture } from './capture';

export interface Req {
  requestId: string;
  /** Absent: the whole page. */
  nodeId?: string | undefined;
  scale: number;
  maxPixels: number;
  minSeq: number;
  uploadToken: string;
  timeoutMs: number;
}

export interface CaptureDeps {
  /** The box in world units of the node, or of all the content for no id; null when it is not there. */
  box(nodeId?: string): Rect | null;
  render(box: Rect, scale: number): Promise<Blob>;
  /** The last server sequence number the file has applied. */
  seq(): number;
  /** Resolves true when the file reaches `minSeq`, false when `timeoutMs` pass first. */
  whenSeq(minSeq: number, timeoutMs: number): Promise<boolean>;
  /** The HTTP status of the upload. */
  upload(requestId: string, headers: Record<string, string>, body: Blob): Promise<number>;
  fail(requestId: string, code: string, message: string): void;
}

export async function serveCapture(r: Req, d: CaptureDeps): Promise<void> {
  const plan = planCapture(r, d.box(r.nodeId));
  if (!plan.ok) return d.fail(r.requestId, plan.code, plan.code === 'too_large' ? 'The image would be larger than the limit.' : 'There is no such node on the canvas.');
  if (!(await d.whenSeq(r.minSeq, r.timeoutMs))) return d.fail(r.requestId, 'stale_client', 'The editor has not received the file up to that change yet.');
  try {
    // The node can have been deleted or resized while it waited: plan again on what is there now.
    const box = d.box(r.nodeId);
    const now = planCapture(r, box);
    if (!now.ok || !box) return d.fail(r.requestId, now.ok ? 'node_not_found' : now.code, 'The node changed while the editor waited.');
    const seq = d.seq();
    const png = await d.render(box, now.scale);
    const status = await d.upload(r.requestId, captureHeaders(r, seq), png);
    if (status !== 200) d.fail(r.requestId, 'internal', `The upload was refused (${status}).`);
  } catch (e) {
    d.fail(r.requestId, 'internal', e instanceof Error ? e.message : 'The image could not be drawn.');
  }
}
