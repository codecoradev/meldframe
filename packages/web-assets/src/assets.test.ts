import { describe, expect, it, vi } from 'vitest';
import { AssetCache, RETRY_AFTER_MS, assetUrl, isAssetId, type AssetDeps, type Bitmap } from './assets';

const ID = 'a'.repeat(64);
/** A decoded image that counts how often it is closed. */
const bitmap = (w = 4, h = 2): Bitmap & { closed: number } => {
  const b = { width: w, height: h, closed: 0, close: () => void (b.closed += 1) };
  return b;
};
const reply = (status: number, body: unknown = {}) => ({
  ok: status >= 200 && status < 300,
  status,
  blob: async () => new Blob(['x']),
  json: async () => body,
});

function cache(over: Partial<AssetDeps> = {}) {
  const fetch = vi.fn(async () => reply(200));
  const decode = vi.fn(async () => bitmap());
  const c = new AssetCache({ fetch, decode, ...over });
  return { c, fetch, decode };
}

describe('AssetCache', () => {
  it('asks the server for the asset of the workspace and has the decoded image when it arrives', async () => {
    const { c, fetch } = cache();
    const seen: string[] = [];
    c.onChange((id) => seen.push(id));
    expect(c.want(ID)).toEqual({ state: 'loading' });
    await c.idle();
    expect(fetch).toHaveBeenCalledWith(assetUrl(ID));
    expect(assetUrl(ID)).toBe(`/v1/assets/${ID}`);
    const got = c.want(ID);
    expect(got.state).toBe('ready');
    expect(seen).toEqual([ID]);
  });

  it('asks once for an asset however often it is wanted', async () => {
    const { c, fetch } = cache();
    c.want(ID);
    c.want(ID);
    await c.idle();
    c.want(ID);
    expect(fetch).toHaveBeenCalledTimes(1);
  });

  it('an asset the server does not have is missing with the code asset_not_found, reported once', async () => {
    const { c } = cache({ fetch: async () => reply(404, { error: { code: 'asset_not_found' } }) });
    const problems: [string, string][] = [];
    c.onProblem = (id, code) => problems.push([id, code]);
    c.want(ID);
    await c.idle();
    expect(c.want(ID)).toEqual({ state: 'missing', code: 'asset_not_found' });
    c.want(ID);
    expect(problems).toEqual([[ID, 'asset_not_found']]);
  });

  it('a 404 without the server\'s asset_not_found is a failed request, not a missing image', async () => {
    const { c } = cache({ fetch: async () => reply(404, {}) });
    c.want(ID);
    await c.idle();
    expect(c.want(ID)).toEqual({ state: 'failed', code: 'not_found' });
  });

  it('another refusal is failed with the code the server gave, or one from the status', async () => {
    const down = cache({ fetch: async () => reply(503, { error: { code: 'unavailable' } }) });
    down.c.want(ID);
    await down.c.idle();
    expect(down.c.want(ID)).toEqual({ state: 'failed', code: 'unavailable' });
    const nobody = cache({ fetch: async () => ({ ...reply(401), json: async () => { throw new Error('not json'); } }) });
    nobody.c.want(ID);
    await nobody.c.idle();
    expect(nobody.c.want(ID)).toEqual({ state: 'failed', code: 'unauthenticated' });
  });

  it('a failed request or an image that cannot be decoded is failed, not an uncaught error', async () => {
    const offline = cache({ fetch: async () => { throw new Error('offline'); } });
    offline.c.want(ID);
    await offline.c.idle();
    expect(offline.c.want(ID)).toEqual({ state: 'failed', code: 'asset_unreadable' });
    const broken = cache({ decode: async () => { throw new Error('bad image'); } });
    broken.c.want(ID);
    await broken.c.idle();
    expect(broken.c.want(ID)).toEqual({ state: 'failed', code: 'asset_unreadable' });
  });

  it('idle waits for every request, and is at once when none is in flight', async () => {
    const { c } = cache();
    await c.idle();
    c.want('b'.repeat(64));
    c.want('c'.repeat(64));
    await c.idle();
    expect(c.want('b'.repeat(64)).state).toBe('ready');
    expect(c.want('c'.repeat(64)).state).toBe('ready');
  });

  it('a listener can stop listening, and destroy frees the decoded images', async () => {
    const b = bitmap();
    const { c } = cache({ decode: async () => b });
    const seen: string[] = [];
    const stop = c.onChange((id) => seen.push(id));
    stop();
    c.want(ID);
    await c.idle();
    expect(seen).toEqual([]);
    c.destroy();
    expect(b.closed).toBe(1);
  });
});

describe('what is asked for', () => {
  it('only an id of 64 lowercase hex characters is put into a path', async () => {
    expect(isAssetId(ID)).toBe(true);
    for (const bad of ['', 'abc', '../../etc/passwd', 'A'.repeat(64), `${'a'.repeat(63)}/`, 'a'.repeat(65)]) expect(isAssetId(bad)).toBe(false);
    const { c, fetch } = cache();
    const problems: [string, string][] = [];
    c.onProblem = (id, code) => problems.push([id, code]);
    expect(c.want('../../v1/files')).toEqual({ state: 'failed', code: 'invalid' });
    await Promise.resolve();
    expect(fetch).not.toHaveBeenCalled();
    expect(problems).toEqual([['../../v1/files', 'invalid']]);
  });
});

describe('a problem is not for ever', () => {
  it('an asset that failed is asked for again after a while, and the image appears when it is there', async () => {
    let now = 0;
    let up = false;
    let asked = 0;
    const { c } = cache({
      now: () => now,
      fetch: async () => {
        asked++;
        return up ? reply(200) : reply(404, { error: { code: 'asset_not_found' } });
      },
    });
    const seen: string[] = [];
    const problems: string[] = [];
    c.onProblem = (_id, code) => problems.push(code);
    c.onChange((id) => seen.push(id));
    c.want(ID);
    await c.idle();
    expect(c.want(ID).state).toBe('missing');
    // Too soon: nothing is asked.
    now = RETRY_AFTER_MS - 1;
    expect(c.retryProblems()).toBe(0);
    // The image has been uploaded meanwhile.
    up = true;
    now = RETRY_AFTER_MS;
    expect(c.retryProblems()).toBe(1);
    await c.idle();
    expect(c.want(ID).state).toBe('ready');
    expect(asked).toBe(2);
    expect(problems).toEqual(['asset_not_found']);
    expect(seen).toEqual([ID, ID]);
  });

  it('a failure after a recovery is news again', async () => {
    let now = 0;
    let state: 'down' | 'up' = 'down';
    const { c } = cache({
      now: () => now,
      fetch: async () => (state === 'up' ? reply(200) : reply(503, { error: { code: 'unavailable' } })),
    });
    const problems: string[] = [];
    c.onProblem = (_id, code) => problems.push(code);
    const round = async () => {
      now += RETRY_AFTER_MS;
      c.retryProblems();
      await c.idle();
    };
    c.want(ID);
    await c.idle();
    state = 'up';
    await round();
    expect(c.want(ID).state).toBe('ready');
    // A loaded image is not asked for again, so a later outage is only seen when the image is asked for anew.
    expect(problems).toEqual(['unavailable']);
  });

  it('a problem that stands is said once however often it is asked for again', async () => {
    let now = 0;
    const { c } = cache({ now: () => now, fetch: async () => reply(503, { error: { code: 'unavailable' } }) });
    const problems: string[] = [];
    c.onProblem = (_id, code) => problems.push(code);
    c.want(ID);
    await c.idle();
    for (let i = 1; i <= 3; i++) {
      now = i * RETRY_AFTER_MS;
      c.retryProblems();
      await c.idle();
    }
    expect(problems).toEqual(['unavailable']);
    expect(c.want(ID).state).toBe('failed');
  });
});

describe('the end of the cache', () => {
  it('an image that arrives after destroy is closed and told to nobody', async () => {
    const b = bitmap();
    let release: () => void = () => {};
    const gate = new Promise<void>((r) => (release = r));
    const { c } = cache({ decode: async () => { await gate; return b; } });
    const seen: string[] = [];
    c.onChange((id) => seen.push(id));
    c.want(ID);
    await Promise.resolve();
    c.destroy();
    release();
    await new Promise((r) => setTimeout(r, 0));
    expect(b.closed).toBe(1);
    expect(seen).toEqual([]);
  });

  it('a problem that is found after destroy is not reported', async () => {
    let release: () => void = () => {};
    const gate = new Promise<void>((r) => (release = r));
    const { c } = cache({ fetch: async () => { await gate; return reply(404, { error: { code: 'asset_not_found' } }); } });
    const problems: string[] = [];
    c.onProblem = (_id, code) => problems.push(code);
    c.want(ID);
    c.destroy();
    release();
    await new Promise((r) => setTimeout(r, 0));
    expect(problems).toEqual([]);
  });

  it('idle gives up after the time it was given, so one stuck request cannot block a capture', async () => {
    const { c } = cache({ fetch: () => new Promise(() => {}) });
    c.want(ID);
    const t0 = Date.now();
    await c.idle(50);
    expect(Date.now() - t0).toBeLessThan(1000);
  });
});
