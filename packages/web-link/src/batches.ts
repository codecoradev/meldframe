// What the shell does for the Activity panel (E5.1, ADR-0021): it maps the `batch` and `touch` frames of the live
// socket to the app layer's frames, and the answers of the list and review routes to actions. Everything here comes
// from the network and is trusted for nothing: each field is checked for type and bound, only the fields the app
// layer knows are copied (it refuses the rest), and a code is capped before it reaches the app. Pure functions; the
// request itself goes through `Env`.
import type { Action, BatchHeader, BatchPage, Frame, ReviewState } from '@meldframe/protocol';

const U32 = 2 ** 32 - 1;
/** The server's `rev` never gets near this; a larger one would pin its row in the app (ADR-0021). */
const MAX_REV = 2 ** 48;
const MAX_ACTOR = 256;
const MAX_PREFIX = 200;
const MAX_TOOL = 100;
/** A note is 500 characters; a character is at most two UTF-16 units. The app cuts and checks it again. */
const MAX_NOTE = 2000;
/** The list route's page size is 1 to 100. */
const MAX_PAGE = 100;
const MAX_CODE = 64;
/** The server sends at most 64 node ids in a touch; a longer list is not from it. */
const MAX_TOUCH_IDS = 64;
const REVIEWS: readonly string[] = ['unreviewed', 'accepted', 'needs_review'];
const FAILED: Action = { type: 'batchRequestFailed', code: 'unavailable' };

type Rec = Record<string, unknown>;
const rec = (v: unknown): Rec | null => (typeof v === 'object' && v !== null && !Array.isArray(v) ? (v as Rec) : null);
/** An own property only: a value that sits on the prototype chain is not the sender's. */
const own = (o: Rec, k: string): unknown => (Object.prototype.hasOwnProperty.call(o, k) ? o[k] : undefined);
const int = (v: unknown, min: number, max: number): v is number => typeof v === 'number' && Number.isInteger(v) && v >= min && v <= max;
const str = (v: unknown, max: number): v is string => typeof v === 'string' && v.length >= 1 && v.length <= max;

/** A batch header from a frame or a row of the list: null when anything is missing, mistyped or out of bound. */
function header(o: Rec): BatchHeader | null {
  const n = own(o, 'n'), rev = own(o, 'rev'), actor = own(o, 'actor'), keyPrefix = own(o, 'keyPrefix'), tool = own(o, 'tool');
  const commands = own(o, 'commands'), written = own(o, 'written'), atomic = own(o, 'atomic');
  const firstSeq = own(o, 'firstSeq'), lastSeq = own(o, 'lastSeq'), atMs = own(o, 'atMs');
  const note = own(o, 'note'), review = own(o, 'review'), reviewedBy = own(o, 'reviewedBy'), reviewedAtMs = own(o, 'reviewedAtMs');
  const big = Number.MAX_SAFE_INTEGER;
  if (!int(n, 1, U32) || !int(rev, 0, MAX_REV) || !str(actor, MAX_ACTOR) || !str(keyPrefix, MAX_PREFIX) || !str(tool, MAX_TOOL)) return null;
  if (!int(commands, 0, U32) || !int(written, 0, U32) || typeof atomic !== 'boolean') return null;
  if (!int(firstSeq, 0, big) || !int(lastSeq, 0, big) || !int(atMs, 0, big)) return null;
  if (typeof review !== 'string' || !REVIEWS.includes(review)) return null;
  if (note !== undefined && (typeof note !== 'string' || note.length > MAX_NOTE)) return null;
  if (reviewedBy !== undefined && !str(reviewedBy, MAX_ACTOR)) return null;
  if (reviewedAtMs !== undefined && !int(reviewedAtMs, 0, big)) return null;
  return {
    n, rev, actor, keyPrefix, tool, commands, written, atomic, firstSeq, lastSeq, atMs,
    ...(note !== undefined ? { note } : {}),
    review: review as ReviewState,
    ...(reviewedBy !== undefined ? { reviewedBy } : {}),
    ...(reviewedAtMs !== undefined ? { reviewedAtMs } : {}),
  };
}

/** The flat `batch` frame of the socket as `Frame::Batch`: the header nested, `type` and `room` dropped. */
export function toBatchFrame(m: unknown): Extract<Frame, { type: 'batch' }> | null {
  const o = rec(m);
  if (!o) return null;
  const unreviewedCount = own(o, 'unreviewedCount');
  const h = header(o);
  if (!h || !int(unreviewedCount, 0, U32)) return null;
  return { type: 'batch', header: h, unreviewedCount };
}

/** The `touch` frame of the socket as `Frame::Touch` (`nodeIds` is `ids` there); null without a sequence number. */
export function toTouchFrame(m: unknown): Extract<Frame, { type: 'touch' }> | null {
  const o = rec(m);
  if (!o) return null;
  const actor = own(o, 'actor'), ids = own(o, 'nodeIds'), seq = own(o, 'seq');
  if (typeof actor !== 'string' || !Array.isArray(ids) || ids.length > MAX_TOUCH_IDS || !ids.every((x) => typeof x === 'string') || !int(seq, 0, Number.MAX_SAFE_INTEGER)) return null;
  return { type: 'touch', actor, ids: ids as string[], seq };
}

/** The code of an error answer: a short snake_case word, else `unavailable`. The body may come from a proxy. */
export function safeCode(v: unknown): string {
  return typeof v === 'string' && v.length <= MAX_CODE && /^[a-z][a-z0-9_]*$/.test(v) ? v : 'unavailable';
}

const failure = (body: unknown): Action => {
  const o = rec(body);
  const e = o ? rec(own(o, 'error')) : null;
  return { type: 'batchRequestFailed', code: safeCode(e ? own(e, 'code') : undefined) };
};

/** The answer of `GET /v1/files/{file}/batches` as `batchesLoaded`, or the failure it stands for. */
export function answerOfList(status: number, body: unknown): Action {
  if (status !== 200) return failure(body);
  const o = rec(body);
  const rows = o ? own(o, 'batches') : undefined;
  const hasMore = o ? own(o, 'hasMore') : undefined, count = o ? own(o, 'unreviewedCount') : undefined, can = o ? own(o, 'canReview') : undefined;
  if (!Array.isArray(rows) || rows.length > MAX_PAGE || typeof hasMore !== 'boolean' || !int(count, 0, U32) || typeof can !== 'boolean') return FAILED;
  const batches: BatchHeader[] = [];
  for (const r of rows) {
    const h = rec(r) && header(rec(r)!);
    if (!h) return FAILED;
    batches.push(h);
  }
  const page: BatchPage = { batches, hasMore, unreviewedCount: count, canReview: can };
  return { type: 'batchesLoaded', page };
}

/** The answer of `PUT .../review`, `{batch, unreviewedCount}`, as `batchReviewed` with the row alone (the frame carries the count). */
export function answerOfReview(status: number, body: unknown): Action {
  if (status !== 200) return failure(body);
  const o = rec(body);
  const row = o ? rec(own(o, 'batch')) : null;
  const h = row && header(row);
  return h ? { type: 'batchReviewed', batch: h } : FAILED;
}

/** The query of `GET .../batches` for a `fetchBatches` effect; a bound that is null is left out. */
export function listQuery(e: { beforeN: number | null; afterN: number | null; limit: number }): string {
  const q = new URLSearchParams();
  if (e.beforeN !== null) q.set('before', String(e.beforeN));
  if (e.afterN !== null) q.set('after', String(e.afterN));
  q.set('limit', String(e.limit));
  return `?${q}`;
}
