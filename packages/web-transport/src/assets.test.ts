// The web-transport seam of the image picker (E2.5, ADR-0015 follow-up): the `fetchAssets` effect is a GET of the asset
// list, and its answer becomes `assetsLoaded` (or `assetsRequestFailed`). The answer comes from the network and is trusted
// for nothing: each field is checked for type and bound, and only the fields the app layer knows are copied.
import type { Action, AppState, Effect } from '@meldframe/protocol';
import { describe, expect, it } from 'vitest';
import { answerOfAssets, assetsQuery } from './assets';
import { Shell, type Engine, type Env } from './shell';

const id = (n: number) => n.toString(16).padStart(2, '0').repeat(32);
const row = (n: number, over: Record<string, unknown> = {}) => ({ id: id(n), bytes: 120 + n, mime: 'image/png', ...over });
const ok = (rows: unknown[], next: unknown = null) => ({ assets: rows, next });

describe('assetsQuery', () => {
  it('carries the limit, and the cursor when there is one', () => {
    expect(assetsQuery({ after: null, limit: 50 })).toBe('?limit=50');
    expect(assetsQuery({ after: id(2), limit: 50 })).toBe(`?limit=50&after=${id(2)}`);
  });
  it('sends no cursor that is not an asset id, and a limit within 1 to 200', () => {
    expect(assetsQuery({ after: '../x', limit: 50 })).toBe('?limit=50');
    expect(assetsQuery({ after: null, limit: 0 })).toBe('?limit=1');
    expect(assetsQuery({ after: null, limit: 5000 })).toBe('?limit=200');
    expect(assetsQuery({ after: null, limit: 2.7 })).toBe('?limit=2');
  });
});

describe('answerOfAssets', () => {
  it('a list answer becomes assetsLoaded with the cursor it was asked with', () => {
    expect(answerOfAssets(7, null, 200, ok([row(1), row(2)], id(2)))).toEqual({
      type: 'assetsLoaded',
      request: 7,
      after: null,
      page: { assets: [row(1), row(2)], next: id(2) },
    });
    expect(answerOfAssets(7, id(2), 200, ok([row(3)]))).toEqual({ type: 'assetsLoaded', request: 7, after: id(2), page: { assets: [row(3)], next: null } });
  });
  it('copies the fields the app knows and drops the others (the app refuses unknown fields)', () => {
    const a = answerOfAssets(7, null, 200, ok([row(1, { name: 'x', extra: 1 })]));
    expect(a).toEqual({ type: 'assetsLoaded', request: 7, after: null, page: { assets: [row(1)], next: null } });
  });
  it('an empty page is a page', () => {
    expect(answerOfAssets(7, null, 200, ok([]))).toEqual({ type: 'assetsLoaded', request: 7, after: null, page: { assets: [], next: null } });
  });
  it('a page that is wrong in shape, bound or content is a failed request', () => {
    const failed: Action = { type: 'assetsRequestFailed', request: 7, code: 'unavailable' };
    const bad: unknown[] = [
      null, 'x', [], {}, { assets: 'x', next: null }, ok([null]), ok(['x']), ok([row(1, { id: 'ABC' })]), ok([row(1, { id: id(171).toUpperCase() })]),
      ok([row(1, { id: `${id(1)}0` })]), ok([row(1, { id: '../../etc/passwd' })]), ok([row(1, { bytes: -1 })]), ok([row(1, { bytes: 1.5 })]),
      ok([row(1, { bytes: '5' })]), ok([row(1, { bytes: Number.MAX_SAFE_INTEGER + 1 })]), ok([row(1, { mime: 'image/svg+xml' })]),
      ok([row(1, { mime: 'text/html' })]), ok([row(1, { mime: undefined })]), ok([row(1)], 'nope'), ok([row(1)], 5), ok([row(1)], id(171).toUpperCase()),
      ok(Array.from({ length: 201 }, (_, i) => row(i))),
    ];
    for (const b of bad) expect(answerOfAssets(7, null, 200, b), JSON.stringify(b)?.slice(0, 80)).toEqual(failed);
  });
  it('does not take a field from the prototype chain', () => {
    const inherited = Object.create(ok([row(1)])) as unknown;
    expect(answerOfAssets(7, null, 200, inherited)).toEqual({ type: 'assetsRequestFailed', request: 7, code: 'unavailable' });
  });
  it('a refusal carries the code of the error, capped to a short word', () => {
    expect(answerOfAssets(7, null, 401, { error: { code: 'unauthenticated', message: 'm' } })).toEqual({ type: 'assetsRequestFailed', request: 7, code: 'unauthenticated' });
    expect(answerOfAssets(7, null, 503, { error: { code: 'unavailable', retryable: true } })).toEqual({ type: 'assetsRequestFailed', request: 7, code: 'unavailable' });
    expect(answerOfAssets(7, null, 500, { error: { code: 'Not A Code!' } })).toEqual({ type: 'assetsRequestFailed', request: 7, code: 'unavailable' });
    expect(answerOfAssets(7, null, 502, '<html>bad gateway</html>')).toEqual({ type: 'assetsRequestFailed', request: 7, code: 'unavailable' });
  });
});

class FakeEngine implements Engine {
  dispatched: Action[] = [];
  private handler: (fx: Effect[]) => void = () => {};
  snapshot() { return { state: { connection: { type: 'live' } } as unknown as AppState }; }
  subscribe() { return () => {}; }
  onEffects(fn: (fx: Effect[]) => void) { this.handler = fn; }
  dispatch(a: Action): Effect[] { this.dispatched.push(a); return []; }
  effect(...fx: Effect[]) { this.handler(fx); }
}
class FakeEnv implements Env {
  requests: { method: string; path: string; body?: unknown; timeoutMs?: number }[] = [];
  reply: () => Promise<{ status: number; body: unknown }> = async () => ({ status: 200, body: null });
  now = () => 1000;
  openSocket = () => { throw new Error('no socket'); };
  post = async () => ({ status: 200, body: {} });
  request = async (method: 'GET' | 'PUT', path: string, body?: unknown, timeoutMs?: number) => {
    this.requests.push({ method, path, body, timeoutMs });
    return this.reply();
  };
  setTimer = () => 1;
  clearTimer = () => {};
}
const settle = () => new Promise((r) => setTimeout(r, 0));
const setup = () => {
  const engine = new FakeEngine();
  const env = new FakeEnv();
  const shell = new Shell(engine, env, { file: 'f' });
  return { engine, env, shell };
};

describe('the fetchAssets effect', () => {
  it('is a GET of /v1/assets with a timeout, with no body, answered as assetsLoaded', async () => {
    const { engine, env } = setup();
    env.reply = async () => ({ status: 200, body: ok([row(1)], id(1)) });
    engine.effect({ type: 'fetchAssets', after: null, limit: 50, request: 4 });
    await settle();
    expect(env.requests).toHaveLength(1);
    expect(env.requests[0]).toMatchObject({ method: 'GET', path: '/v1/assets?limit=50', body: undefined });
    expect(env.requests[0]!.timeoutMs).toBeGreaterThan(0);
    expect(engine.dispatched).toEqual([{ type: 'assetsLoaded', request: 4, after: null, page: { assets: [row(1)], next: id(1) } }]);
  });
  it('answers with the request number and the cursor it was made with', async () => {
    const { engine, env } = setup();
    env.reply = async () => ({ status: 200, body: ok([row(2)]) });
    engine.effect({ type: 'fetchAssets', after: id(1), limit: 50, request: 5 });
    await settle();
    expect(env.requests[0]!.path).toBe(`/v1/assets?limit=50&after=${id(1)}`);
    expect(engine.dispatched).toEqual([{ type: 'assetsLoaded', request: 5, after: id(1), page: { assets: [row(2)], next: null } }]);
  });
  it('a refusal and a network failure become assetsRequestFailed', async () => {
    const { engine, env } = setup();
    env.reply = async () => ({ status: 401, body: { error: { code: 'unauthenticated', message: 'm' } } });
    engine.effect({ type: 'fetchAssets', after: null, limit: 50, request: 4 });
    await settle();
    env.reply = async () => { throw new Error('offline'); };
    engine.effect({ type: 'fetchAssets', after: null, limit: 50, request: 4 });
    await settle();
    expect(engine.dispatched).toEqual([
      { type: 'assetsRequestFailed', request: 4, code: 'unauthenticated' },
      { type: 'assetsRequestFailed', request: 4, code: 'unavailable' },
    ]);
  });
  it('an answer that comes after stop is not dispatched', async () => {
    const { engine, env, shell } = setup();
    shell.start();
    engine.dispatched.length = 0;
    engine.effect({ type: 'fetchAssets', after: null, limit: 50, request: 4 });
    shell.stop();
    await settle();
    expect(engine.dispatched).toEqual([]);
    expect(env.requests).toHaveLength(1);
  });
});
