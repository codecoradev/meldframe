// The web-transport seam of the image picker (E2.5, ADR-0015 follow-up): the `fetchAssets` effect is a GET of the asset list,
// and the status and the text of its answer go to the app layer as they came (`Action.wire`). Reading the page is the app
// layer's (finding T6): the table of what a page may hold is in answers_assets.rs.
import type { Action, AppState, Effect } from '@meldframe/protocol';
import { describe, expect, it } from 'vitest';
import { assetsQuery } from './assets';
import { Shell, type Engine, type Env } from './shell';

const id = (n: number) => n.toString(16).padStart(2, '0').repeat(32);

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
  reply: () => Promise<{ status: number; text: string }> = async () => ({ status: 200, text: '' });
  now = () => 1000;
  openSocket = () => { throw new Error('no socket'); };
  post = async () => ({ status: 200, text: '{}' });
  request = async (method: 'GET' | 'PUT' | 'POST', path: string, body?: unknown, timeoutMs?: number) => {
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
  it('is a GET of /v1/assets with a timeout, with no body, answered with the raw text', async () => {
    const { engine, env } = setup();
    const text = `{"assets":[],"next":"${id(1)}"}`;
    env.reply = async () => ({ status: 200, text });
    engine.effect({ type: 'fetchAssets', after: null, limit: 50, request: 4 });
    await settle();
    expect(env.requests).toHaveLength(1);
    expect(env.requests[0]).toMatchObject({ method: 'GET', path: '/v1/assets?limit=50', body: undefined });
    expect(env.requests[0]!.timeoutMs).toBeGreaterThan(0);
    expect(engine.dispatched).toEqual([{ type: 'wire', wire: { kind: 'assets', request: 4, after: null, status: 200, body: text } }]);
  });
  it('answers with the request number and the cursor it was made with', async () => {
    const { engine, env } = setup();
    env.reply = async () => ({ status: 200, text: '{}' });
    engine.effect({ type: 'fetchAssets', after: id(1), limit: 50, request: 5 });
    await settle();
    expect(env.requests[0]!.path).toBe(`/v1/assets?limit=50&after=${id(1)}`);
    expect(engine.dispatched).toEqual([{ type: 'wire', wire: { kind: 'assets', request: 5, after: id(1), status: 200, body: '{}' } }]);
  });
  it('a refusal is handed over as it is, and a network failure becomes assetsRequestFailed', async () => {
    const { engine, env } = setup();
    env.reply = async () => ({ status: 401, text: '{"error":{"code":"unauthenticated","message":"m"}}' });
    engine.effect({ type: 'fetchAssets', after: null, limit: 50, request: 4 });
    await settle();
    env.reply = async () => { throw new Error('offline'); };
    engine.effect({ type: 'fetchAssets', after: null, limit: 50, request: 4 });
    await settle();
    expect(engine.dispatched).toEqual([
      { type: 'wire', wire: { kind: 'assets', request: 4, after: null, status: 401, body: '{"error":{"code":"unauthenticated","message":"m"}}' } },
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
