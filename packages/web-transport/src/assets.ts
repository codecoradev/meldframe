// What the shell does for the image picker (E2.5, ADR-0015 follow-up): the query of `GET /v1/assets` for a `fetchAssets`
// effect, and its answer as an action. The answer comes from the network and is trusted for nothing: each field is checked
// for type and bound, only the fields the app layer knows are copied (it refuses the rest), and a code is capped before
// it reaches the app. Pure functions; the request itself goes through `Env`. There is no upload here: before sign-in an
// agent uploads through `upload_asset`, and the picker lists what the workspace holds.
import type { Action, AssetInfo } from '@meldframe/protocol';
import { safeCode } from './batches';

/** The route's page size is 1 to 200. */
const MAX_PAGE = 200;
/** The lowercase hex SHA-256 the server names an asset by. */
const ID = /^[0-9a-f]{64}$/;
/** The four types the server serves (SVG is refused at upload). */
const MIMES: readonly string[] = ['image/png', 'image/jpeg', 'image/webp', 'image/gif'];

type Rec = Record<string, unknown>;
const rec = (v: unknown): Rec | null => (typeof v === 'object' && v !== null && !Array.isArray(v) ? (v as Rec) : null);
/** An own property only: a value that sits on the prototype chain is not the sender's. */
const own = (o: Rec, k: string): unknown => (Object.prototype.hasOwnProperty.call(o, k) ? o[k] : undefined);
const isId = (v: unknown): v is string => typeof v === 'string' && ID.test(v);

/** The query of `GET /v1/assets` for a `fetchAssets` effect: the limit held to 1 to 200, and a cursor only when it is an asset id. */
export function assetsQuery(e: { after: string | null; limit: number }): string {
  const q = new URLSearchParams();
  q.set('limit', String(Math.min(MAX_PAGE, Math.max(1, Math.trunc(e.limit) || 1))));
  if (e.after !== null && isId(e.after)) q.set('after', e.after);
  return `?${q}`;
}

/** The failure of request number `request`, with a code (the app keeps it and takes it only for the request that is out). */
export const assetsFailed = (request: number, code = 'unavailable'): Action => ({ type: 'assetsRequestFailed', request, code });

/** The answer of `GET /v1/assets` as `assetsLoaded` (with the request number and the cursor the request carried), or the failure it stands for. */
export function answerOfAssets(request: number, after: string | null, status: number, body: unknown): Action {
  const FAILED = assetsFailed(request);
  const o = rec(body);
  if (status !== 200) {
    const e = o ? rec(own(o, 'error')) : null;
    return assetsFailed(request, safeCode(e ? own(e, 'code') : undefined));
  }
  const rows = o ? own(o, 'assets') : undefined;
  const next = o ? own(o, 'next') : undefined;
  if (!Array.isArray(rows) || rows.length > MAX_PAGE || (next !== null && !isId(next))) return FAILED;
  const assets: AssetInfo[] = [];
  for (const r of rows) {
    const a = rec(r);
    const id = a ? own(a, 'id') : undefined, bytes = a ? own(a, 'bytes') : undefined, mime = a ? own(a, 'mime') : undefined;
    if (!isId(id) || typeof bytes !== 'number' || !Number.isInteger(bytes) || bytes < 0 || bytes > Number.MAX_SAFE_INTEGER) return FAILED;
    if (typeof mime !== 'string' || !MIMES.includes(mime)) return FAILED;
    assets.push({ id, bytes, mime });
  }
  return { type: 'assetsLoaded', request, after, page: { assets, next } };
}
