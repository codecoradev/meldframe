// The web-transport seam of the checkpoints (E5.3, ADR-0023): the flat `checkpoint` frame of the socket becomes the app
// layer's `Frame::Checkpoint`, the answer of the list route becomes an action, and the read of the list is carried out
// with a fake fetch. Nothing from the network is trusted: shape and bounds are checked here.
import type { Action, AppState, Effect } from '@meldframe/protocol';
import { describe, expect, it, vi } from 'vitest';
import { answerOfRows, checkpointCall, toCheckpointFrame } from './checkpoints';
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
});

describe('checkpointCall', () => {
  it('a read of the list is a GET of the collection', () => {
    expect(checkpointCall('file dev', { type: 'fetchCheckpoints' })).toMatchObject({ method: 'GET', path: '/v1/files/file%20dev/checkpoints' });
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
  it('a refusal and a network failure tell the app that the read failed', async () => {
    const { engine, env } = setup();
    env.reply = async () => ({ status: 403, body: { error: { code: 'forbidden', message: 'm' } } });
    engine.effect({ type: 'fetchCheckpoints' });
    await settle();
    env.reply = async () => { throw new Error('offline'); };
    engine.effect({ type: 'fetchCheckpoints' });
    await settle();
    expect(engine.dispatched).toEqual([
      { type: 'checkpointRequestFailed', request: { type: 'list' }, code: 'forbidden' },
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
  it('an answer the app cannot take is told to it as the failure of the read, so the read is never left out', async () => {
    const { engine, env } = setup();
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    env.reply = async () => ({ status: 200, body: { checkpoints: [], canCreate: true } });
    const take = engine.dispatch.bind(engine);
    engine.dispatch = (a) => {
      if (a.type === 'checkpointsLoaded') throw new Error('the app refused the action');
      return take(a);
    };
    engine.effect({ type: 'fetchCheckpoints' });
    await settle();
    expect(engine.dispatched).toEqual([{ type: 'checkpointRequestFailed', request: { type: 'list' }, code: 'unavailable' }]);
    expect(warn).toHaveBeenCalledTimes(1);
    warn.mockRestore();
  });
  it('until the transport carries them out, a post and a read of the state are told to the app as failed, so a revert or a restore that waits for them is refused and not left waiting', async () => {
    const { engine, env } = setup();
    engine.effect({ type: 'postCheckpoint', key: 'k1', kind: 'beforeRevert', label: null, about: null, seq: 3 } as Effect);
    engine.effect({ type: 'fetchCheckpointState', n: 7 } as Effect);
    await settle();
    expect(engine.dispatched).toEqual([
      { type: 'checkpointRequestFailed', request: { type: 'post', key: 'k1' }, code: 'unavailable' },
      { type: 'checkpointRequestFailed', request: { type: 'state', n: 7 }, code: 'unavailable' },
    ]);
    expect(env.calls).toHaveLength(0);
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
