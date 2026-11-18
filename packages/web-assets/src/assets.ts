// The images that image fills point at, fetched from the server and kept decoded. A node draws without its image until
// the image is `ready`; an asset the server does not have is `missing` (`asset_not_found`), and any other failure is
// `failed` with its code. Each is asked for once; the canvas draws again when one arrives (`onChange`), and a problem is
// reported once (`onProblem`). A problem is not for ever: the server may have been restarting, or the image may be
// uploaded later, so `retryProblems` asks again for those that failed a while ago. The transport and the clock are
// injected, so this is tested without a browser.

export type AssetState =
  | { state: 'loading' }
  | { state: 'ready'; bitmap: Bitmap }
  | { state: 'missing'; code: string }
  | { state: 'failed'; code: string };

/** What the painter needs of a decoded image. */
export interface Bitmap {
  width: number;
  height: number;
  close?: () => void;
}

export interface AssetDeps {
  fetch: (url: string) => Promise<{ ok: boolean; status: number; blob: () => Promise<Blob>; json: () => Promise<unknown> }>;
  decode: (blob: Blob) => Promise<Bitmap>;
  /** Milliseconds, for the time a problem has stood; the default is the wall clock. */
  now?: () => number;
}

/** An asset id: the lowercase hex SHA-256 of its bytes. Anything else is not asked for, whatever the document holds. */
export const isAssetId = (id: string): boolean => /^[0-9a-f]{64}$/.test(id);

/** Where the server gives an asset of the workspace of the token (through the dev proxy, which adds the token). */
export const assetUrl = (id: string): string => `/v1/assets/${id}`;

/** How long a problem stands before `retryProblems` asks again. */
export const RETRY_AFTER_MS = 10_000;

export class AssetCache {
  private entries = new Map<string, AssetState>();
  /** When each problem was recorded. */
  private failedAt = new Map<string, number>();
  private reported = new Set<string>();
  private pending = new Set<Promise<void>>();
  private listeners = new Set<(id: string) => void>();
  private destroyed = false;
  /** Called once for an asset that is missing or cannot be loaded, with the stable code. */
  onProblem: ((id: string, code: string) => void) | null = null;

  constructor(private deps: AssetDeps) {}

  private now(): number {
    return (this.deps.now ?? Date.now)();
  }

  /** The state of an asset, and the request for it when nobody has asked yet. */
  want(id: string): AssetState {
    const known = this.entries.get(id);
    if (known) return known;
    if (!isAssetId(id)) {
      // Never put a value of the document into a path unchecked: the server takes 64 hex characters and no more.
      const bad: AssetState = { state: 'failed', code: 'invalid' };
      this.entries.set(id, bad);
      this.failedAt.set(id, this.now());
      queueMicrotask(() => this.report(id, 'invalid'));
      return bad;
    }
    return this.start(id);
  }

  private start(id: string): AssetState {
    const loading: AssetState = { state: 'loading' };
    this.entries.set(id, loading);
    const job = this.load(id).finally(() => this.pending.delete(job));
    this.pending.add(job);
    return loading;
  }

  /** Asks again for every asset whose problem is older than `RETRY_AFTER_MS`; how many were asked for. */
  retryProblems(): number {
    let n = 0;
    for (const [id, at] of [...this.failedAt]) {
      if (this.now() - at < RETRY_AFTER_MS || !isAssetId(id)) continue;
      this.failedAt.delete(id);
      this.start(id);
      n++;
    }
    return n;
  }

  /** Calls `fn` with the id of each asset that arrives or fails. Returns the function that stops it. */
  onChange(fn: (id: string) => void): () => void {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  }

  /** Resolves when no request is in flight, or after `maxMs` (a capture waits for the pictures, but not for ever). */
  async idle(maxMs = Infinity): Promise<void> {
    const until = this.now() + maxMs;
    while (this.pending.size > 0 && this.now() < until) {
      const wait = Promise.all([...this.pending]);
      if (maxMs === Infinity) {
        await wait;
        continue;
      }
      let timer: ReturnType<typeof setTimeout> | undefined;
      const limit = new Promise((r) => (timer = setTimeout(r, Math.max(0, until - this.now()))));
      try {
        await Promise.race([wait, limit]);
      } finally {
        clearTimeout(timer);
      }
    }
  }

  /** Frees the decoded images; what arrives afterwards is closed and told to nobody. */
  destroy(): void {
    this.destroyed = true;
    for (const e of this.entries.values()) if (e.state === 'ready') e.bitmap.close?.();
    this.entries.clear();
    this.failedAt.clear();
    this.listeners.clear();
    this.onProblem = null;
  }

  private report(id: string, code: string): void {
    if (this.destroyed || this.reported.has(id)) return;
    this.reported.add(id);
    this.onProblem?.(id, code);
    for (const fn of this.listeners) fn(id);
  }

  private settle(id: string, next: AssetState): void {
    if (this.destroyed) {
      if (next.state === 'ready') next.bitmap.close?.();
      return;
    }
    this.entries.set(id, next);
    if (next.state === 'missing' || next.state === 'failed') {
      this.failedAt.set(id, this.now());
      if (this.reported.has(id)) for (const fn of this.listeners) fn(id);
      else this.report(id, next.code);
      return;
    }
    this.failedAt.delete(id);
    // It is there now: a later failure of the same image is news again.
    this.reported.delete(id);
    for (const fn of this.listeners) fn(id);
  }

  private async load(id: string): Promise<void> {
    try {
      const res = await this.deps.fetch(assetUrl(id));
      if (!res.ok) {
        const code = await codeOf(res);
        // Only the server's own `asset_not_found` is a missing asset; a 404 with no such body (a server without assets)
        // is a failure of the request, not a statement about this image.
        if (res.status === 404 && code === 'asset_not_found') this.settle(id, { state: 'missing', code });
        else this.settle(id, { state: 'failed', code: code ?? (res.status === 404 ? 'not_found' : 'unavailable') });
        return;
      }
      const bitmap = await this.deps.decode(await res.blob());
      this.settle(id, { state: 'ready', bitmap });
    } catch {
      this.settle(id, { state: 'failed', code: 'asset_unreadable' });
    }
  }
}

/** The error code in the body of a refused call (`{"error": {"code"}}`), else one from the status (401), else none. */
async function codeOf(res: { status: number; json: () => Promise<unknown> }): Promise<string | undefined> {
  try {
    const body = (await res.json()) as { error?: { code?: unknown } };
    if (typeof body.error?.code === 'string') return body.error.code;
  } catch {
    // not JSON: fall through
  }
  return res.status === 401 ? 'unauthenticated' : undefined;
}
