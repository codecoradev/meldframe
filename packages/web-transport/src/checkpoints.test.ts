// The web-transport seam of the checkpoints (E5.3, ADR-0023): the flat `checkpoint` frame of the socket becomes the app
// layer's `Frame::Checkpoint`, the answers of the list, post and state routes become actions, and the three effects are
// carried out with a fake fetch. Nothing from the network is trusted: shape and bounds are checked here.
import type { Action, AppState, Effect } from '@meldframe/protocol';
import { describe, expect, it, vi } from 'vitest';
import { answerOfPost, answerOfRows, answerOfState, checkpointCall, toCheckpointFrame } from './checkpoints';
import { Shell, type Engine, type Env, type SocketLike } from './shell';

const wire = (over: Record<string, unknown> = {}) => ({ type: 'checkpoint', room: 'file_dev', n: 3, seq: 12, kind: 'manual', label: 'v1', by: 'user:bea', atMs: 5, ...over });
const row = { n: 3, seq: 12, kind: 'manual', label: 'v1', by: 'user:bea', atMs: 5 };
const run = { n: 4, seq: 12, kind: 'runStart', about: 'agent:x', atMs: 6 };

describe('toCheckpointFrame', () => {
  it('nests the row and drops type and room', () => {
    expect(toCheckpointFrame(wire())).toEqual({ type: 'checkpoint', checkpoint: row });
    expect(toCheckpointFrame(wire({ n: 4, kind: 'runStart', label: undefined, by: undefined, about: 'agent:x', atMs: 6 }))?.checkpoint).toEqual(run);
  });
  it('drops the fields it does not know (the app refuses them)', () => {
    expect(toCheckpointFrame(wire({ extra: 1 }))?.checkpoint).toEqual(row);
  });
  it('refuses a frame with a missing, mistyped or out of bound field', () => {
    const bad: Record<string, unknown>[] = [
      { n: 0 }, { n: 1.5 }, { n: 2 ** 32 }, { n: '3' }, { seq: -1 }, { seq: Number.MAX_SAFE_INTEGER + 1 }, { kind: 'snapshot' }, { kind: 5 },
      { atMs: -1 }, { atMs: undefined }, { label: 5 }, { label: 'x'.repeat(2001) }, { about: '' }, { about: 'a'.repeat(300) }, { by: 1 }, { by: 'b'.repeat(300) },
    ];
    for (const b of bad) expect(toCheckpointFrame(wire(b)), JSON.stringify(b)).toBeNull();
    expect(toCheckpointFrame(null)).toBeNull();
    expect(toCheckpointFrame('x')).toBeNull();
  });
  it('does not take a field from the prototype chain', () => {
    expect(toCheckpointFrame(Object.create(wire()))).toBeNull();
  });
});

describe('the answers', () => {
  const failed = (request: object, code: string): Action => ({ type: 'checkpointRequestFailed', request, code }) as Action;
  it('the list is checkpointsLoaded with canCreate', () => {
    expect(answerOfRows(200, { checkpoints: [run, row], canCreate: true })).toEqual({ type: 'checkpointsLoaded', checkpoints: [run, row], canCreate: true });
    expect(answerOfRows(200, { checkpoints: [], canCreate: false })).toEqual({ type: 'checkpointsLoaded', checkpoints: [], canCreate: false });
  });
  it('a list that is not one is a failure of the read', () => {
    const list = failed({ type: 'list' }, 'unavailable');
    expect(answerOfRows(200, { checkpoints: [{ ...row, n: 0 }], canCreate: true })).toEqual(list);
    expect(answerOfRows(200, { checkpoints: [row] })).toEqual(list);
    expect(answerOfRows(200, { checkpoints: Array.from({ length: 101 }, () => row), canCreate: true })).toEqual(list);
    expect(answerOfRows(200, null)).toEqual(list);
  });
  it('a server without the module answers 404 with no error body: not_found, which the app reads as none', () => {
    expect(answerOfRows(404, '<html>')).toEqual(failed({ type: 'list' }, 'not_found'));
    expect(answerOfRows(403, { error: { code: 'forbidden', message: 'm' } })).toEqual(failed({ type: 'list' }, 'forbidden'));
    expect(answerOfRows(500, { error: { code: 'x'.repeat(5000) } })).toEqual(failed({ type: 'list' }, 'unavailable'));
  });
  it('a post answers with its row and the key it was sent with', () => {
    expect(answerOfPost('k1')(200, { checkpoint: row, created: true })).toEqual({ type: 'checkpointPosted', key: 'k1', checkpoint: row });
    expect(answerOfPost('k1')(200, { created: true })).toEqual(failed({ type: 'post', key: 'k1' }, 'unavailable'));
  });
  it('a refused post carries the code of the error', () => {
    for (const code of ['checkpoint_full', 'key_reused', 'invalid', 'forbidden']) {
      expect(answerOfPost('k1')(409, { error: { code, message: 'm', n: 2 } })).toEqual(failed({ type: 'post', key: 'k1' }, code));
    }
  });
  it('the state is checkpointStateLoaded with the number it was asked for', () => {
    const node = { id: 'a', type: 'rect' };
    expect(answerOfState(3)(200, { seq: 12, nodes: [node] })).toEqual({ type: 'checkpointStateLoaded', n: 3, seq: 12, nodes: [node] });
    for (const body of [{ seq: -1, nodes: [] }, { seq: 1 }, { seq: 1, nodes: [1] }, { seq: 1, nodes: 'x' }, null]) {
      expect(answerOfState(3)(200, body), JSON.stringify(body)).toEqual(failed({ type: 'state', n: 3 }, 'unavailable'));
    }
    expect(answerOfState(3)(422, { error: { code: 'too_far', message: 'm' } })).toEqual(failed({ type: 'state', n: 3 }, 'too_far'));
  });
});

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
  reply: () => Promise<{ status: number; body: unknown }> = async () => ({ status: 200, body: null });
  now = () => 1000;
  openSocket = () => { const s = new FakeSocket(); this.sockets.push(s); return s; };
  post = async (path: string, body: unknown, timeoutMs?: number) => { this.calls.push({ method: 'POST', path, body, timeoutMs }); return this.reply(); };
  request = async (method: 'GET' | 'PUT', path: string, body?: unknown, timeoutMs?: number) => { this.calls.push({ method, path, body, timeoutMs }); return this.reply(); };
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
  it('fetchCheckpoints is a GET answered as checkpointsLoaded', async () => {
    const { engine, env } = setup();
    env.reply = async () => ({ status: 200, body: { checkpoints: [row], canCreate: true } });
    engine.effect({ type: 'fetchCheckpoints' });
    await settle();
    expect(env.calls).toHaveLength(1);
    expect(env.calls[0]).toMatchObject({ method: 'GET', path: '/v1/files/f/checkpoints' });
    expect(env.calls[0]!.timeoutMs).toBeGreaterThan(0);
    expect(engine.dispatched).toEqual([{ type: 'checkpointsLoaded', checkpoints: [row], canCreate: true }]);
  });
  it('postCheckpoint is a POST with the key, answered as checkpointPosted with that key', async () => {
    const { engine, env } = setup();
    env.reply = async () => ({ status: 200, body: { checkpoint: row, created: true } });
    engine.effect({ type: 'postCheckpoint', key: 'k1', kind: 'manual', label: 'v1', about: null, seq: 12 });
    await settle();
    expect(env.calls[0]).toMatchObject({ method: 'POST', path: '/v1/files/f/checkpoints', body: { key: 'k1', kind: 'manual', seq: 12, label: 'v1' } });
    expect(engine.dispatched).toEqual([{ type: 'checkpointPosted', key: 'k1', checkpoint: row }]);
  });
  it('fetchCheckpointState is a GET answered as checkpointStateLoaded', async () => {
    const { engine, env } = setup();
    env.reply = async () => ({ status: 200, body: { seq: 12, nodes: [] } });
    engine.effect({ type: 'fetchCheckpointState', n: 3 });
    await settle();
    expect(env.calls[0]).toMatchObject({ method: 'GET', path: '/v1/files/f/checkpoints/3/state' });
    expect(engine.dispatched).toEqual([{ type: 'checkpointStateLoaded', n: 3, seq: 12, nodes: [] }]);
  });
  it('a refusal and a network failure tell the app which request failed', async () => {
    const { engine, env } = setup();
    env.reply = async () => ({ status: 422, body: { error: { code: 'too_far', message: 'm' } } });
    engine.effect({ type: 'fetchCheckpointState', n: 3 });
    await settle();
    env.reply = async () => { throw new Error('offline'); };
    engine.effect({ type: 'postCheckpoint', key: 'k9', kind: 'beforeRestore', label: null, about: null, seq: 1 }, { type: 'fetchCheckpoints' });
    await settle();
    expect(engine.dispatched).toEqual([
      { type: 'checkpointRequestFailed', request: { type: 'state', n: 3 }, code: 'too_far' },
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
    const answers = ['checkpointsLoaded', 'checkpointPosted', 'checkpointStateLoaded'];
    engine.dispatch = (a) => {
      if (answers.includes(a.type)) throw new Error('the app refused the action');
      return take(a);
    };
    env.reply = async () => ({ status: 200, body: { checkpoints: [], canCreate: true } });
    engine.effect({ type: 'fetchCheckpoints' });
    await settle();
    env.reply = async () => ({ status: 200, body: { checkpoint: row, created: true } });
    engine.effect({ type: 'postCheckpoint', key: 'k7', kind: 'beforeRevert', label: null, about: 'agent:x', seq: 3 });
    await settle();
    env.reply = async () => ({ status: 200, body: { seq: 12, nodes: [] } });
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
    env.reply = async () => ({ status: 200, body: { checkpoints: [], canCreate: true } });
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

describe('the frame on the socket', () => {
  const live = () => {
    const { engine, env, shell } = setup('file_dev');
    shell.start();
    engine.effect({ type: 'subscribe', afterSeq: 0 });
    engine.dispatched.length = 0;
    return { engine, socket: env.sockets[0]! };
  };
  it('a checkpoint frame is dispatched as Frame::Checkpoint', () => {
    const { engine, socket } = live();
    socket.onmessage(JSON.stringify(wire()));
    expect(engine.dispatched).toEqual([{ type: 'frame', nowMs: 1000, frame: { type: 'checkpoint', checkpoint: row } }]);
    expect(socket.closed).toBe(false);
  });
  it('a malformed checkpoint frame is a lost hint: ignored, the connection stays', () => {
    const { engine, socket } = live();
    socket.onmessage(JSON.stringify(wire({ kind: 'x' })));
    socket.onmessage(JSON.stringify(wire({ n: 0 })));
    expect(engine.dispatched).toEqual([]);
    expect(socket.closed).toBe(false);
  });
  it('a frame the app cannot take costs the connection, as any frame does', () => {
    const { engine, socket } = live();
    const take = engine.dispatch.bind(engine);
    engine.dispatch = (a) => {
      if (a.type === 'frame') throw new Error('the app refused the frame');
      return take(a);
    };
    socket.onmessage(JSON.stringify(wire()));
    expect(socket.closed).toBe(true);
  });
});
