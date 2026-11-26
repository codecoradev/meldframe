// The web-transport seam of the Activity panel (E5.1, ADR-0021 slice 11): the two effects are carried out with a fake fetch,
// and the status and the text of the answer, and of a `batch` or `touch` frame, go to the app layer as they came (`Action.wire`).
// Reading them is the app layer's (finding T6): the tables of what a body may hold are in answers_batches.rs.
import type { Action, AppState, Effect } from '@meldframe/protocol';
import { describe, expect, it, vi } from 'vitest';
import { listQuery } from './batches';
import { Shell, type Engine, type Env, type SocketLike } from './shell';

const page = '{"batches":[],"hasMore":false,"unreviewedCount":1,"canReview":true}';

class FakeSocket implements SocketLike {
  sent: string[] = [];
  closed = false;
  onopen = () => {};
  onmessage: (data: string) => void = () => {};
  onclose = () => {};
  send(s: string) { this.sent.push(s); }
  close() { this.closed = true; }
}
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
  sockets: FakeSocket[] = [];
  requests: { method: string; path: string; body?: unknown; timeoutMs?: number }[] = [];
  reply: () => Promise<{ status: number; text: string }> = async () => ({ status: 200, text: '' });
  now = () => 1000;
  openSocket = () => { const s = new FakeSocket(); this.sockets.push(s); return s; };
  post = async () => ({ status: 200, body: {} });
  request = async (method: 'GET' | 'PUT' | 'POST', path: string, body?: unknown, timeoutMs?: number) => {
    this.requests.push({ method, path, body, timeoutMs });
    return this.reply();
  };
  setTimer = () => 1;
  clearTimer = () => {};
}
const settle = () => new Promise((r) => setTimeout(r, 0));
const setup = (file = 'file dev') => {
  const engine = new FakeEngine();
  const env = new FakeEnv();
  const shell = new Shell(engine, env, { file });
  return { engine, env, shell };
};

describe('listQuery', () => {
  it('leaves a bound that is null out, and always carries the limit', () => {
    expect(listQuery({ beforeN: 9, afterN: null, limit: 50 })).toBe('?before=9&limit=50');
    expect(listQuery({ beforeN: 9, afterN: 2, limit: 100 })).toBe('?before=9&after=2&limit=100');
    expect(listQuery({ beforeN: null, afterN: null, limit: 50 })).toBe('?limit=50');
  });
});

describe('the effects', () => {
  it('fetchBatches is a GET with the bounds the app asked for, answered with the status and the text as they came', async () => {
    const { engine, env } = setup();
    env.reply = async () => ({ status: 200, text: page });
    engine.effect({ type: 'fetchBatches', beforeN: 9, afterN: null, limit: 50 });
    await settle();
    expect(env.requests).toHaveLength(1);
    expect(env.requests[0]).toMatchObject({ method: 'GET', path: '/v1/files/file%20dev/batches?before=9&limit=50' });
    expect(env.requests[0]!.timeoutMs).toBeGreaterThan(0);
    expect(engine.dispatched).toEqual([{ type: 'wire', wire: { kind: 'batches', status: 200, body: page } }]);
  });
  it('fetchBatches with both bounds sends both; with none sends the limit only', async () => {
    const { engine, env } = setup('f');
    engine.effect({ type: 'fetchBatches', beforeN: 9, afterN: 2, limit: 100 }, { type: 'fetchBatches', beforeN: null, afterN: null, limit: 50 });
    await settle();
    expect(env.requests.map((r) => r.path)).toEqual(['/v1/files/f/batches?before=9&after=2&limit=100', '/v1/files/f/batches?limit=50']);
  });
  it('putBatchReview is a PUT of the state, answered as the raw review answer', async () => {
    const { engine, env } = setup('f');
    const text = '{"batch":{},"unreviewedCount":0}';
    env.reply = async () => ({ status: 200, text });
    engine.effect({ type: 'putBatchReview', n: 3, state: 'accepted' });
    await settle();
    expect(env.requests[0]).toMatchObject({ method: 'PUT', path: '/v1/files/f/batches/3/review', body: { state: 'accepted' } });
    expect(engine.dispatched).toEqual([{ type: 'wire', wire: { kind: 'batchReview', status: 200, body: text } }]);
  });
  it('a refusal, a page of a proxy and an empty body are handed over as they are; a network failure is batchRequestFailed', async () => {
    const { engine, env } = setup('f');
    env.reply = async () => ({ status: 403, text: '{"error":{"code":"forbidden","message":"m"}}' });
    engine.effect({ type: 'putBatchReview', n: 3, state: 'accepted' });
    await settle();
    env.reply = async () => ({ status: 502, text: '<html>bad gateway</html>' });
    engine.effect({ type: 'fetchBatches', beforeN: null, afterN: null, limit: 50 });
    await settle();
    env.reply = async () => { throw new Error('offline'); };
    engine.effect({ type: 'fetchBatches', beforeN: null, afterN: null, limit: 50 });
    await settle();
    expect(engine.dispatched).toEqual([
      { type: 'wire', wire: { kind: 'batchReview', status: 403, body: '{"error":{"code":"forbidden","message":"m"}}' } },
      { type: 'wire', wire: { kind: 'batches', status: 502, body: '<html>bad gateway</html>' } },
      { type: 'batchRequestFailed', code: 'unavailable' },
    ]);
  });
  it('an answer that comes after stop is not dispatched', async () => {
    const { engine, env, shell } = setup('f');
    shell.start();
    engine.dispatched.length = 0;
    engine.effect({ type: 'fetchBatches', beforeN: null, afterN: null, limit: 50 });
    shell.stop();
    await settle();
    expect(engine.dispatched).toEqual([]);
    expect(env.requests).toHaveLength(1);
  });
  it('an answer the app cannot take is warned about once, and the queue goes on', async () => {
    const { engine, env } = setup('f');
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    env.reply = async () => ({ status: 200, text: page });
    engine.dispatch = () => { throw new Error('refused'); };
    for (let i = 0; i < 3; i++) {
      engine.effect({ type: 'fetchBatches', beforeN: null, afterN: null, limit: 50 });
      await settle();
    }
    expect(env.requests).toHaveLength(3);
    expect(warn).toHaveBeenCalledTimes(1);
    warn.mockRestore();
  });
});

describe('frames on the socket', () => {
  const live = () => {
    const { engine, env, shell } = setup('file_dev');
    shell.start();
    engine.effect({ type: 'subscribe', afterSeq: 0 });
    engine.dispatched.length = 0;
    return { engine, socket: env.sockets[0]! };
  };
  const batch = '{"type":"batch","room":"file_dev","n":3,"rev":7,"actor":"agent:claude","keyPrefix":"k1","tool":"apply_commands","commands":4,"written":4,"atomic":true,"firstSeq":10,"lastSeq":13,"atMs":5000,"review":"unreviewed","unreviewedCount":2}';
  it('a batch, a touch and a checkpoint frame go to the app layer as the text the socket sent, with the time', () => {
    const { engine, socket } = live();
    const touch = '{"type":"touch","actor":"agent:x","nodeIds":["a","b"],"seq":4}';
    const checkpoint = '{"type":"checkpoint","room":"f","n":3,"seq":12,"kind":"manual","atMs":5}';
    for (const text of [batch, touch, checkpoint]) socket.onmessage(text);
    expect(engine.dispatched).toEqual([batch, touch, checkpoint].map((text) => ({ type: 'wire', wire: { kind: 'hint', nowMs: 1000, text } })));
    expect(socket.closed).toBe(false);
  });
  it('a frame the shell does not read is not checked here either: a malformed hint is the app layer to ignore, and the connection stays', () => {
    const { engine, socket } = live();
    socket.onmessage('{"type":"batch","n":-1}');
    expect(engine.dispatched).toEqual([{ type: 'wire', wire: { kind: 'hint', nowMs: 1000, text: '{"type":"batch","n":-1}' } }]);
    expect(socket.closed).toBe(false);
  });
  it('a hint the app cannot take costs the connection, as any frame does', () => {
    const { engine, socket } = live();
    const take = engine.dispatch.bind(engine);
    engine.dispatch = (a) => {
      if (a.type === 'wire') throw new Error('the app refused the frame');
      return take(a);
    };
    socket.onmessage(batch);
    expect(socket.closed).toBe(true);
    expect(engine.dispatched).toEqual([{ type: 'disconnected' }]);
  });
});
