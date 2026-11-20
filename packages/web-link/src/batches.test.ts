// The web-link seam of the Activity panel (E5.1, ADR-0021 slice 11): the flat `batch` frame of the socket becomes the
// app layer's `Frame::Batch`, the answers of the list and review routes become actions, and the two effects are
// carried out with a fake fetch. Nothing from the network is trusted: shape and bounds are checked here.
import type { Action, AppState, Effect } from '@meldframe/protocol';
import { describe, expect, it, vi } from 'vitest';
import { answerOfList, answerOfReview, safeCode, toBatchFrame, toTouchFrame } from './batches';
import { Shell, type Engine, type Env, type SocketLike } from './shell';

const wire = (over: Record<string, unknown> = {}) => ({
  type: 'batch', room: 'file_dev', n: 3, rev: 7, actor: 'agent:claude', keyPrefix: 'k1', tool: 'apply_commands', commands: 4,
  written: 4, atomic: true, firstSeq: 10, lastSeq: 13, atMs: 5000, review: 'unreviewed', unreviewedCount: 2, ...over,
});
const header = {
  n: 3, rev: 7, actor: 'agent:claude', keyPrefix: 'k1', tool: 'apply_commands', commands: 4, written: 4, atomic: true,
  firstSeq: 10, lastSeq: 13, atMs: 5000, review: 'unreviewed',
};

describe('toBatchFrame', () => {
  it('nests the header, drops type and room, and lifts the count', () => {
    expect(toBatchFrame(wire())).toEqual({ type: 'batch', header, unreviewedCount: 2 });
  });
  it('keeps the optional fields it knows and drops the ones it does not (the app refuses unknown fields)', () => {
    const f = toBatchFrame(wire({ note: 'why', reviewedBy: 'person:a', reviewedAtMs: 9, review: 'needs_review', extra: 1 }));
    expect(f?.header).toEqual({ ...header, note: 'why', reviewedBy: 'person:a', reviewedAtMs: 9, review: 'needs_review' });
  });
  it('refuses a frame with a missing, mistyped or out of bound field', () => {
    const bad: Record<string, unknown>[] = [
      { n: 0 }, { n: 1.5 }, { n: 2 ** 32 }, { rev: -1 }, { rev: 2 ** 49 }, { rev: '7' }, { actor: '' }, { actor: 5 }, { actor: 'a'.repeat(300) },
      { keyPrefix: 'k'.repeat(201) }, { tool: 3 }, { commands: -1 }, { written: 1e10 }, { atomic: 'yes' }, { firstSeq: Number.MAX_SAFE_INTEGER + 1 },
      { atMs: null }, { review: 'approved' }, { review: undefined }, { note: 7 }, { note: 'n'.repeat(2001) }, { reviewedBy: 1 },
      { reviewedAtMs: -2 }, { unreviewedCount: -1 }, { unreviewedCount: 2 ** 32 }, { unreviewedCount: undefined },
    ];
    for (const b of bad) expect(toBatchFrame(wire(b)), JSON.stringify(b)).toBeNull();
    expect(toBatchFrame(null)).toBeNull();
    expect(toBatchFrame('x')).toBeNull();
  });
  it('does not take a field from the prototype chain', () => {
    const m = Object.create(wire()) as Record<string, unknown>; // every field is inherited, none is the sender's
    expect(toBatchFrame(m)).toBeNull();
  });
});

describe('toTouchFrame', () => {
  it('maps nodeIds to ids and needs a sequence number', () => {
    expect(toTouchFrame({ type: 'touch', actor: 'a', nodeIds: ['x'], seq: 4 })).toEqual({ type: 'touch', actor: 'a', ids: ['x'], seq: 4 });
    expect(toTouchFrame({ type: 'touch', actor: 'a', nodeIds: ['x'] })).toBeNull();
    expect(toTouchFrame({ type: 'touch', actor: 'a', nodeIds: ['x'], seq: -1 })).toBeNull();
  });
  it('takes at most 64 ids (the server sends no more) and refuses a longer list', () => {
    const ids = (n: number) => Array.from({ length: n }, (_, i) => `n${i}`);
    expect(toTouchFrame({ type: 'touch', actor: 'a', nodeIds: ids(64), seq: 1 })?.ids).toHaveLength(64);
    expect(toTouchFrame({ type: 'touch', actor: 'a', nodeIds: ids(65), seq: 1 })).toBeNull();
  });
});

describe('safeCode', () => {
  it('passes a short snake_case code and replaces anything else', () => {
    expect(safeCode('forbidden')).toBe('forbidden');
    expect(safeCode('rate_limited')).toBe('rate_limited');
    for (const v of ['x'.repeat(65), '', 'Has Space', 'a\nb', '<script>', 'ünï', 7, null, undefined, {}]) expect(safeCode(v), String(v)).toBe('unavailable');
  });
});

describe('answers of the routes', () => {
  const page = { batches: [{ ...header, extra: 1 }], hasMore: true, unreviewedCount: 1, canReview: false, extra: 2 };
  it('a list answer becomes batchesLoaded, with the fields the app knows', () => {
    expect(answerOfList(200, page)).toEqual({
      type: 'batchesLoaded',
      page: { batches: [header], hasMore: true, unreviewedCount: 1, canReview: false },
    });
  });
  it('a list answer that is wrong in shape or too long is a failed request', () => {
    const fail = { type: 'batchRequestFailed', code: 'unavailable' };
    expect(answerOfList(200, null)).toEqual(fail);
    expect(answerOfList(200, { ...page, hasMore: 1 })).toEqual(fail);
    expect(answerOfList(200, { ...page, batches: [{ ...header, review: 'x' }] })).toEqual(fail);
    expect(answerOfList(200, { ...page, batches: Array.from({ length: 101 }, () => header) })).toEqual(fail);
    expect(answerOfList(200, { ...page, canReview: undefined })).toEqual(fail);
  });
  it('a review answer is unwrapped to the row alone', () => {
    expect(answerOfReview(200, { batch: { ...header, review: 'accepted', reviewedBy: 'p', reviewedAtMs: 1 }, unreviewedCount: 0 })).toEqual({
      type: 'batchReviewed',
      batch: { ...header, review: 'accepted', reviewedBy: 'p', reviewedAtMs: 1 },
    });
    expect(answerOfReview(200, { batch: null, unreviewedCount: 0 })).toEqual({ type: 'batchRequestFailed', code: 'unavailable' });
    expect(answerOfReview(200, header)).toEqual({ type: 'batchRequestFailed', code: 'unavailable' });
  });
  it('a refusal carries the code of the error, capped', () => {
    expect(answerOfList(403, { error: { code: 'forbidden', message: 'm' } })).toEqual({ type: 'batchRequestFailed', code: 'forbidden' });
    expect(answerOfReview(404, { error: { code: 'not_found' } })).toEqual({ type: 'batchRequestFailed', code: 'not_found' });
    expect(answerOfList(500, { error: { code: 'x'.repeat(5000) } })).toEqual({ type: 'batchRequestFailed', code: 'unavailable' });
    expect(answerOfList(502, '<html>')).toEqual({ type: 'batchRequestFailed', code: 'unavailable' });
  });
});

class FakeSocket implements SocketLike {
  sent: string[] = [];
  closed = false;
  onopen = () => {};
  onmessage = (_d: string) => {};
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
  reply: () => Promise<{ status: number; body: unknown }> = async () => ({ status: 200, body: null });
  now = () => 1000;
  openSocket = () => { const s = new FakeSocket(); this.sockets.push(s); return s; };
  post = async () => ({ status: 200, body: {} });
  request = async (method: 'GET' | 'PUT', path: string, body?: unknown, timeoutMs?: number) => {
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

describe('the effects', () => {
  it('fetchBatches is a GET with the bounds the app asked for, answered as batchesLoaded', async () => {
    const { engine, env } = setup();
    env.reply = async () => ({ status: 200, body: { batches: [header], hasMore: false, unreviewedCount: 1, canReview: true } });
    engine.effect({ type: 'fetchBatches', beforeN: 9, afterN: null, limit: 50 });
    await settle();
    expect(env.requests).toHaveLength(1);
    expect(env.requests[0]).toMatchObject({ method: 'GET', path: '/v1/files/file%20dev/batches?before=9&limit=50' });
    expect(env.requests[0]!.timeoutMs).toBeGreaterThan(0);
    expect(engine.dispatched).toEqual([{ type: 'batchesLoaded', page: { batches: [header], hasMore: false, unreviewedCount: 1, canReview: true } }]);
  });
  it('fetchBatches with both bounds sends both; with none sends the limit only', async () => {
    const { engine, env } = setup('f');
    engine.effect({ type: 'fetchBatches', beforeN: 9, afterN: 2, limit: 100 }, { type: 'fetchBatches', beforeN: null, afterN: null, limit: 50 });
    await settle();
    expect(env.requests.map((r) => r.path)).toEqual(['/v1/files/f/batches?before=9&after=2&limit=100', '/v1/files/f/batches?limit=50']);
  });
  it('putBatchReview is a PUT of the state, answered as batchReviewed with the row alone', async () => {
    const { engine, env } = setup('f');
    const row = { ...header, review: 'accepted', reviewedBy: 'p', reviewedAtMs: 1 };
    env.reply = async () => ({ status: 200, body: { batch: row, unreviewedCount: 0 } });
    engine.effect({ type: 'putBatchReview', n: 3, state: 'accepted' });
    await settle();
    expect(env.requests[0]).toMatchObject({ method: 'PUT', path: '/v1/files/f/batches/3/review', body: { state: 'accepted' } });
    expect(engine.dispatched).toEqual([{ type: 'batchReviewed', batch: row }]);
  });
  it('a refusal and a network failure become batchRequestFailed', async () => {
    const { engine, env } = setup('f');
    env.reply = async () => ({ status: 403, body: { error: { code: 'forbidden', message: 'm' } } });
    engine.effect({ type: 'putBatchReview', n: 3, state: 'accepted' });
    await settle();
    env.reply = async () => { throw new Error('offline'); };
    engine.effect({ type: 'fetchBatches', beforeN: null, afterN: null, limit: 50 });
    await settle();
    expect(engine.dispatched).toEqual([
      { type: 'batchRequestFailed', code: 'forbidden' },
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
    env.reply = async () => ({ status: 200, body: { batches: [header], hasMore: false, unreviewedCount: 1, canReview: true } });
    engine.dispatch = () => { throw new Error('refused'); };
    for (let i = 0; i < 3; i++) {
      engine.effect({ type: 'fetchBatches', beforeN: null, afterN: null, limit: 50 });
      await settle();
    }
    expect(env.requests).toHaveLength(3);
    expect(warn).toHaveBeenCalledTimes(1);
    warn.mockRestore();
  });
  it('a request the app cannot take does not break the effect queue', async () => {
    const { engine, env } = setup('f');
    env.reply = async () => ({ status: 200, body: { batches: [header], hasMore: false, unreviewedCount: 1, canReview: true } });
    engine.dispatch = () => { throw new Error('refused'); };
    engine.effect({ type: 'fetchBatches', beforeN: null, afterN: null, limit: 50 });
    await settle();
    engine.effect({ type: 'fetchBatches', beforeN: null, afterN: null, limit: 50 });
    await settle();
    expect(env.requests).toHaveLength(2);
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
  it('a batch frame is dispatched as Frame::Batch', () => {
    const { engine, socket } = live();
    socket.onmessage(JSON.stringify(wire()));
    expect(engine.dispatched).toEqual([{ type: 'frame', nowMs: 1000, frame: { type: 'batch', header, unreviewedCount: 2 } }]);
    expect(socket.closed).toBe(false);
  });
  it('a malformed batch frame is a lost hint: ignored, the connection stays', () => {
    const { engine, socket } = live();
    socket.onmessage(JSON.stringify(wire({ review: 'x' })));
    socket.onmessage(JSON.stringify(wire({ n: -1 })));
    expect(engine.dispatched).toEqual([]);
    expect(socket.closed).toBe(false);
  });
  it('a batch frame the app cannot take costs the connection, as any frame does', () => {
    const { engine, socket } = live();
    const take = engine.dispatch.bind(engine);
    engine.dispatch = (a) => {
      if (a.type === 'frame') throw new Error('the app refused the frame');
      return take(a);
    };
    socket.onmessage(JSON.stringify(wire()));
    expect(socket.closed).toBe(true);
    expect(engine.dispatched).toEqual([{ type: 'disconnected' }]);
  });
  it('a touch frame is dispatched as Frame::Touch', () => {
    const { engine, socket } = live();
    socket.onmessage('{"type":"touch","actor":"agent:x","nodeIds":["a","b"],"seq":4}');
    expect(engine.dispatched).toEqual([{ type: 'frame', nowMs: 1000, frame: { type: 'touch', actor: 'agent:x', ids: ['a', 'b'], seq: 4 } }]);
  });
});
