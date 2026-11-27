// The web-transport seam of the checkpoints (E5.3, ADR-0023): the three effects are carried out with a fake fetch, and the
// status and the text of the answer go to the app layer as they came (`Action.wire`). Reading them is the app layer's (finding
// T6): the tables of what a body may hold are in answers_checkpoints.rs.
import type { Action, AppState, Effect } from '@meldframe/protocol';
import { describe, expect, it, vi } from 'vitest';
import { checkpointCall } from './checkpoints';
import { Shell, type Engine, type Env, type SocketLike } from './shell';

const list = '{"checkpoints":[],"canCreate":true}';

describe('checkpointCall', () => {
  it('a read of the list is a GET of the collection', () => {
    expect(checkpointCall('file dev', { type: 'fetchCheckpoints' })).toMatchObject({ method: 'GET', path: '/v1/files/file%20dev/checkpoints' });
  });
  it('a post sends the key, the kind, the seq and what it has, and leaves a null out', () => {
    const e: Effect = { type: 'postCheckpoint', key: 'k1', kind: 'manual', label: 'v1', about: null, seq: 4 };
    expect(checkpointCall('f', e)).toMatchObject({ method: 'POST', path: '/v1/files/f/checkpoints', body: { key: 'k1', kind: 'manual', seq: 4, label: 'v1' } });
    const revert: Effect = { type: 'postCheckpoint', key: 'k2', kind: 'beforeRevert', label: null, about: 'agent:x', seq: 4 };
    expect(checkpointCall('f', revert).body).toEqual({ key: 'k2', kind: 'beforeRevert', seq: 4, about: 'agent:x' });
  });
  it('a read of the state is a GET of the number', () => {
    expect(checkpointCall('f', { type: 'fetchCheckpointState', n: 7 })).toMatchObject({ method: 'GET', path: '/v1/files/f/checkpoints/7/state' });
  });
  it('names each raw answer by its request, and each network failure too', () => {
    const post = checkpointCall('f', { type: 'postCheckpoint', key: 'k1', kind: 'manual', label: null, about: null, seq: 1 });
    expect(post.wire(200, 'x')).toEqual({ kind: 'checkpointPosted', key: 'k1', status: 200, body: 'x' });
    expect(post.failure).toEqual({ type: 'checkpointRequestFailed', request: { type: 'post', key: 'k1' }, code: 'unavailable' });
    const state = checkpointCall('f', { type: 'fetchCheckpointState', n: 7 });
    expect(state.wire(422, 'y')).toEqual({ kind: 'checkpointState', n: 7, status: 422, body: 'y' });
    expect(state.failure).toEqual({ type: 'checkpointRequestFailed', request: { type: 'state', n: 7 }, code: 'unavailable' });
    expect(checkpointCall('f', { type: 'fetchCheckpoints' }).wire(404, 'z')).toEqual({ kind: 'checkpoints', status: 404, body: 'z' });
  });
});

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
type Call = { method: string; path: string; body?: unknown; timeoutMs?: number };
class FakeEnv implements Env {
  sockets: FakeSocket[] = [];
  calls: Call[] = [];
  reply: () => Promise<{ status: number; text: string }> = async () => ({ status: 200, text: '' });
  now = () => 1000;
  openSocket = () => { const s = new FakeSocket(); this.sockets.push(s); return s; };
  post = async () => ({ status: 200, text: '{}' });
  request = async (method: 'GET' | 'PUT' | 'POST', path: string, body?: unknown, timeoutMs?: number) => { this.calls.push({ method, path, body, timeoutMs }); return this.reply(); };
  setTimer = () => 1;
  clearTimer = () => {};
}
const settle = () => new Promise((r) => setTimeout(r, 0));
const setup = (file = 'f') => {
  const engine = new FakeEngine();
  const env = new FakeEnv();
  const shell = new Shell(engine, env, { file });
  return { engine, env, shell };
};

describe('the effects', () => {
  it('fetchCheckpoints is a GET answered with the raw text', async () => {
    const { engine, env } = setup();
    env.reply = async () => ({ status: 200, text: list });
    engine.effect({ type: 'fetchCheckpoints' });
    await settle();
    expect(env.calls).toHaveLength(1);
    expect(env.calls[0]).toMatchObject({ method: 'GET', path: '/v1/files/f/checkpoints' });
    expect(env.calls[0]!.timeoutMs).toBeGreaterThan(0);
    expect(engine.dispatched).toEqual([{ type: 'wire', wire: { kind: 'checkpoints', status: 200, body: list } }]);
  });
  it('postCheckpoint is a POST with the key, answered with the raw text and that key', async () => {
    const { engine, env } = setup();
    env.reply = async () => ({ status: 200, text: '{"checkpoint":{},"created":true}' });
    engine.effect({ type: 'postCheckpoint', key: 'k1', kind: 'manual', label: 'v1', about: null, seq: 12 });
    await settle();
    expect(env.calls[0]).toMatchObject({ method: 'POST', path: '/v1/files/f/checkpoints', body: { key: 'k1', kind: 'manual', seq: 12, label: 'v1' } });
    expect(engine.dispatched).toEqual([{ type: 'wire', wire: { kind: 'checkpointPosted', key: 'k1', status: 200, body: '{"checkpoint":{},"created":true}' } }]);
  });
  it('fetchCheckpointState is a GET answered with the raw text and the number', async () => {
    const { engine, env } = setup();
    env.reply = async () => ({ status: 200, text: '{"seq":12,"nodes":[]}' });
    engine.effect({ type: 'fetchCheckpointState', n: 3 });
    await settle();
    expect(env.calls[0]).toMatchObject({ method: 'GET', path: '/v1/files/f/checkpoints/3/state' });
    expect(engine.dispatched).toEqual([{ type: 'wire', wire: { kind: 'checkpointState', n: 3, status: 200, body: '{"seq":12,"nodes":[]}' } }]);
  });
  it('a refusal is handed over as it is, and a network failure tells the app which request failed', async () => {
    const { engine, env } = setup();
    env.reply = async () => ({ status: 422, text: '{"error":{"code":"too_far","message":"m"}}' });
    engine.effect({ type: 'fetchCheckpointState', n: 3 });
    await settle();
    env.reply = async () => { throw new Error('offline'); };
    engine.effect({ type: 'postCheckpoint', key: 'k9', kind: 'beforeRestore', label: null, about: null, seq: 1 }, { type: 'fetchCheckpoints' });
    await settle();
    expect(engine.dispatched).toEqual([
      { type: 'wire', wire: { kind: 'checkpointState', n: 3, status: 422, body: '{"error":{"code":"too_far","message":"m"}}' } },
      { type: 'checkpointRequestFailed', request: { type: 'post', key: 'k9' }, code: 'unavailable' },
      { type: 'checkpointRequestFailed', request: { type: 'list' }, code: 'unavailable' },
    ]);
  });
  it('an answer that comes after stop is not dispatched', async () => {
    const { engine, env, shell } = setup();
    shell.start();
    engine.dispatched.length = 0;
    engine.effect({ type: 'fetchCheckpoints' });
    shell.stop();
    await settle();
    expect(engine.dispatched).toEqual([]);
    expect(env.calls).toHaveLength(1);
  });
  it('an answer the app cannot take is told to it as the failure of its request, so nothing waits for it for ever', async () => {
    const { engine, env } = setup();
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const take = engine.dispatch.bind(engine);
    engine.dispatch = (a) => {
      if (a.type === 'wire') throw new Error('the app refused the action');
      return take(a);
    };
    env.reply = async () => ({ status: 200, text: list });
    engine.effect({ type: 'fetchCheckpoints' });
    await settle();
    engine.effect({ type: 'postCheckpoint', key: 'k7', kind: 'beforeRevert', label: null, about: 'agent:x', seq: 3 });
    await settle();
    engine.effect({ type: 'fetchCheckpointState', n: 3 });
    await settle();
    expect(engine.dispatched).toEqual([
      { type: 'checkpointRequestFailed', request: { type: 'list' }, code: 'unavailable' },
      { type: 'checkpointRequestFailed', request: { type: 'post', key: 'k7' }, code: 'unavailable' },
      { type: 'checkpointRequestFailed', request: { type: 'state', n: 3 }, code: 'unavailable' },
    ]);
    expect(warn).toHaveBeenCalled();
    warn.mockRestore();
  });
  it('a failure the app cannot take either is warned about, and the queue goes on', async () => {
    const { engine, env } = setup();
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    env.reply = async () => ({ status: 200, text: list });
    engine.dispatch = () => { throw new Error('refused'); };
    for (let i = 0; i < 2; i++) {
      engine.effect({ type: 'fetchCheckpoints' });
      await settle();
    }
    expect(env.calls).toHaveLength(2);
    expect(warn).toHaveBeenCalled();
    warn.mockRestore();
  });
});
