// The shell against a scripted engine: no wasm needed, so pull request CI runs these. The same logic against the
// real app layer is in shell.wasm.test.ts (skipped without a wasm build).
import type { Action, AppState, Effect } from '@meldframe/protocol';
import { describe, expect, it } from 'vitest';
import { Shell, parseFrame, type Engine, type Env, type SocketLike } from './shell';

class FakeSocket implements SocketLike {
  sent: string[] = [];
  closed = false;
  onopen = () => {};
  onmessage = (_data: string) => {};
  onclose = () => {};
  constructor(public url: string) {}
  send(s: string) {
    this.sent.push(s);
  }
  close() {
    this.closed = true;
  }
}

/** Records what the shell dispatches, and lets a test say which connection state the app is in. */
class FakeEngine implements Engine {
  dispatched: Action[] = [];
  connection: string = 'connecting';
  /** Makes dispatch throw for the actions it matches (a wasm trap, a refused action). */
  throwOn: ((a: Action) => boolean) | null = null;
  private handler: (fx: Effect[]) => void = () => {};
  private listeners: ((s: { state: AppState }) => void)[] = [];
  snapshot() {
    return { state: { connection: { type: this.connection } } as unknown as AppState };
  }
  subscribe(fn: (s: { state: AppState }) => void) {
    this.listeners.push(fn);
    return () => {};
  }
  onEffects(fn: (fx: Effect[]) => void) {
    this.handler = fn;
  }
  dispatch(a: Action): Effect[] {
    if (this.throwOn?.(a)) throw new Error('the app refused the action');
    this.dispatched.push(a);
    return [];
  }
  /** The app moves to another connection state. */
  moveTo(type: string) {
    this.connection = type;
    for (const fn of this.listeners) fn(this.snapshot());
  }
  /** What the app layer asks for. */
  effect(...fx: Effect[]) {
    this.handler(fx);
  }
}

class FakeEnv implements Env {
  clock = 1000;
  sockets: FakeSocket[] = [];
  posts: { path: string; body: unknown }[] = [];
  reply: () => Promise<{ status: number; body: unknown }> = async () => ({ status: 200, body: {} });
  timers: { at: number; fn: () => void; every?: number; id: number }[] = [];
  private next = 1;
  now = () => this.clock;
  openSocket = (path: string) => {
    const s = new FakeSocket(path);
    this.sockets.push(s);
    return s;
  };
  post = async (path: string, body: unknown) => {
    this.posts.push({ path, body });
    return this.reply();
  };
  setTimer = (fn: () => void, ms: number, repeat = false) => {
    const id = this.next++;
    this.timers.push({ at: this.clock + ms, fn, every: repeat ? ms : undefined, id });
    return id;
  };
  clearTimer = (id: number) => {
    this.timers = this.timers.filter((t) => t.id !== id);
  };
  advance(ms: number) {
    const to = this.clock + ms;
    for (;;) {
      const due = this.timers.filter((t) => t.at <= to).sort((a, b) => a.at - b.at)[0];
      if (!due) break;
      this.clock = due.at;
      if (due.every) due.at += due.every;
      else this.timers = this.timers.filter((t) => t !== due);
      due.fn();
    }
    this.clock = to;
  }
}

const setup = () => {
  const engine = new FakeEngine();
  const env = new FakeEnv();
  const shell = new Shell(engine, env, { file: 'file_dev' });
  return { engine, env, shell };
};
const op = (key: string) => ({ key, command: { type: 'removeNode', id: 'a' } as never, deps: [] });
const settle = () => new Promise((r) => setTimeout(r, 0));

describe('parseFrame', () => {
  it('accepts well-formed frames and refuses the rest', () => {
    expect(parseFrame('{"type":"heartbeat"}')).toEqual({ type: 'heartbeat' });
    expect(parseFrame('{"type":"event","seq":3,"actor":"u","command":{}}')?.seq).toBe(3);
    expect(parseFrame('{"type":"resync","afterSeq":1,"latestSeq":9}')?.latestSeq).toBe(9);
    for (const bad of ['not json', '[]', 'null', '{}', '{"type":5}', '{"type":"event","seq":-1,"actor":"u","command":{}}',
      '{"type":"event","seq":1.5,"actor":"u","command":{}}', '{"type":"event","seq":1,"actor":"u"}', '{"type":"resync","afterSeq":1}']) {
      expect(parseFrame(bad), bad).toBeNull();
    }
  });
});

describe('touch frames', () => {
  const live = (onTouch: (t: { actor: string; nodeIds: string[] }) => void) => {
    const engine = new FakeEngine();
    const env = new FakeEnv();
    const shell = new Shell(engine, env, { file: 'file_dev', onTouch });
    shell.start();
    engine.effect({ type: 'subscribe', afterSeq: 0 });
    return { engine, socket: env.sockets[0]! };
  };
  it('parseFrame takes a touch with an actor and node ids and refuses the rest', () => {
    expect(parseFrame('{"type":"touch","actor":"agent:x","nodeIds":["a"],"seq":3}')?.type).toBe('touch');
    for (const bad of ['{"type":"touch","nodeIds":["a"]}', '{"type":"touch","actor":"u"}', '{"type":"touch","actor":"u","nodeIds":[1]}', '{"type":"touch","actor":"u","nodeIds":"a"}']) {
      expect(parseFrame(bad), bad).toBeNull();
    }
  });
  it('hands a touch to the listener and does not dispatch it to the app', () => {
    const seen: { actor: string; nodeIds: string[] }[] = [];
    const { engine, socket } = live((t) => seen.push(t));
    const before = engine.dispatched.length;
    socket.onmessage('{"type":"touch","actor":"agent:claude","nodeIds":["a","b"],"seq":4}');
    expect(seen).toEqual([{ actor: 'agent:claude', nodeIds: ['a', 'b'] }]);
    expect(engine.dispatched).toHaveLength(before);
  });
  it('a listener that throws does not lose the connection', () => {
    const { engine, socket } = live(() => {
      throw new Error('a view broke');
    });
    const before = engine.dispatched.length;
    socket.onmessage('{"type":"touch","actor":"u","nodeIds":["a"],"seq":1}');
    expect(engine.dispatched).toHaveLength(before); // no reconnect action
    expect(socket.closed).toBe(false);
  });
});

describe('capture requests', () => {
  const req = { type: 'captureRequest', requestId: 'r1', room: 'file_dev', nodeId: 'a', scale: 2, maxPixels: 1000000, minSeq: 3, uploadToken: 'tok', timeoutMs: 5000 };
  const live = (onCapture?: (r: unknown) => void) => {
    const engine = new FakeEngine();
    const env = new FakeEnv();
    const shell = new Shell(engine, env, { file: 'file_dev', ...(onCapture ? { onCapture } : {}) });
    shell.start();
    engine.effect({ type: 'subscribe', afterSeq: 0 });
    return { engine, shell, socket: env.sockets[0]! };
  };
  it('a shell with a capture handler says so after it is subscribed, and one without does not', () => {
    const a = live(() => {});
    a.socket.onmessage('{"type":"subscribed"}');
    expect(a.socket.sent.map((x) => JSON.parse(x)).find((m) => m.type === 'capabilities')).toEqual({ type: 'capabilities', room: 'file_dev', capabilities: ['capture'] });
    const b = live();
    b.socket.onmessage('{"type":"subscribed"}');
    expect(b.socket.sent.map((x) => JSON.parse(x)).some((m) => m.type === 'capabilities')).toBe(false);
  });
  it('hands a request to the handler; a request that lacks a field is not handed over', () => {
    const seen: unknown[] = [];
    const { socket } = live((r) => seen.push(r));
    socket.onmessage('{"type":"subscribed"}');
    socket.onmessage(JSON.stringify(req));
    expect(seen).toEqual([req]);
    socket.onmessage(JSON.stringify({ ...req, uploadToken: undefined }));
    expect(seen).toHaveLength(1);
  });
  it('says it again on the socket of a reconnect', () => {
    const engine = new FakeEngine();
    const env = new FakeEnv();
    const shell = new Shell(engine, env, { file: 'file_dev', onCapture: () => {} });
    shell.start();
    engine.effect({ type: 'subscribe', afterSeq: 0 });
    env.sockets[0]!.onmessage('{"type":"subscribed"}');
    env.sockets[0]!.onclose(); // the connection is lost
    engine.effect({ type: 'subscribe', afterSeq: 4 });
    const second = env.sockets[1]!;
    second.onmessage('{"type":"subscribed"}');
    expect(second.sent.map((x) => JSON.parse(x)).some((m) => m.type === 'capabilities')).toBe(true);
  });
  it('answers a request it cannot serve with captureFailed on the same socket', () => {
    const { shell, socket } = live(() => {});
    socket.onmessage('{"type":"subscribed"}');
    shell.captureFailed('r1', 'node_not_found', 'no such node');
    expect(socket.sent.map((x) => JSON.parse(x)).at(-1)).toEqual({ type: 'captureFailed', requestId: 'r1', code: 'node_not_found', message: 'no such node' });
  });
  it('a handler that throws does not lose the connection', () => {
    const { socket } = live(() => {
      throw new Error('boom');
    });
    socket.onmessage('{"type":"subscribed"}');
    socket.onmessage(JSON.stringify(req));
    expect(socket.closed).toBe(false);
  });
});

describe('Shell with a scripted engine', () => {
  it('start dispatches start and ticks every second', () => {
    const { engine, env, shell } = setup();
    shell.start();
    expect(engine.dispatched).toEqual([{ type: 'start', nowMs: 1000 }]);
    env.advance(2000);
    expect(engine.dispatched.filter((a) => a.type === 'tick')).toHaveLength(2);
  });

  it('subscribe opens the live socket of the file after the given number, and its frames become actions', () => {
    const { engine, env, shell } = setup();
    shell.start();
    engine.effect({ type: 'subscribe', afterSeq: 7 });
    const s = env.sockets[0]!;
    expect(s.url).toBe('/v1/live/file_dev?after=7');
    s.onmessage('{"type":"subscribed"}');
    s.onmessage('{"type":"heartbeat"}');
    s.onmessage('{"type":"event","seq":8,"actor":"u","command":{"type":"removeNode","id":"a"}}');
    const types = engine.dispatched.map((a) => (a.type === 'frame' ? `frame:${a.frame.type}` : a.type));
    expect(types).toEqual(['start', 'connected', 'frame:heartbeat', 'frame:event']);
  });

  it('a malformed frame or a closed socket is a lost connection', () => {
    const { engine, env, shell } = setup();
    shell.start();
    engine.effect({ type: 'subscribe', afterSeq: 0 });
    env.sockets[0]!.onmessage('garbage');
    expect(engine.dispatched.at(-1)).toEqual({ type: 'disconnected' });
    expect(env.sockets[0]!.closed).toBe(true);
    engine.effect({ type: 'subscribe', afterSeq: 0 });
    env.sockets[1]!.onclose();
    expect(engine.dispatched.at(-1)).toEqual({ type: 'disconnected' });
  });

  it('what a dropped socket says afterwards is ignored', () => {
    const { engine, env, shell } = setup();
    shell.start();
    engine.effect({ type: 'subscribe', afterSeq: 0 });
    const old = env.sockets[0]!;
    old.onmessage('garbage'); // drops it
    const count = engine.dispatched.length;
    old.onmessage('{"type":"subscribed"}');
    old.onclose();
    expect(engine.dispatched).toHaveLength(count);
  });

  it('reconnect and wakeAfter wait for the time asked, then dispatch with the clock of that moment', () => {
    const { engine, env, shell } = setup();
    shell.start();
    engine.effect({ type: 'reconnect', afterMs: 500 }, { type: 'wakeAfter', afterMs: 250 });
    env.advance(250);
    expect(engine.dispatched.at(-1)).toEqual({ type: 'tick', nowMs: 1250 });
    env.advance(250);
    expect(engine.dispatched.at(-1)).toEqual({ type: 'reconnect', nowMs: 1500 });
  });

  it('a live socket is pinged every 15 seconds; a socket that cannot send is a lost connection', () => {
    const { engine, env, shell } = setup();
    shell.start();
    engine.effect({ type: 'subscribe', afterSeq: 0 });
    env.advance(15_000);
    expect(env.sockets[0]!.sent.map((x) => JSON.parse(x).type)).toEqual(['ping']);
    env.sockets[0]!.send = () => {
      throw new Error('not open');
    };
    env.advance(15_000);
    expect(engine.dispatched.at(-1)).toEqual({ type: 'disconnected' });
  });

  it('sendOps posts one request per op, in order', async () => {
    const { engine, env, shell } = setup();
    shell.start();
    engine.effect({ type: 'sendOps', ops: [op('k1'), op('k2')] });
    await settle();
    expect(env.posts.map((p) => (p.body as { ops: { key: string }[] }).ops[0]!.key)).toEqual(['k1', 'k2']);
    expect(env.posts[0]!.path).toBe('/v1/files/file_dev/tools/apply_commands');
  });

  it.each(['approval_required', 'approval_denied', 'approval_expired', 'approval_limit'])('%s is passed on as not retryable, so the app rolls back at once', async (code) => {
    const { engine, env, shell } = setup();
    shell.start();
    env.reply = async () => ({ status: code === 'approval_limit' ? 429 : 403, body: { error: { code, retryable: false } } });
    engine.effect({ type: 'sendOps', ops: [op('k1')] });
    await settle();
    expect(engine.dispatched.at(-1)).toMatchObject({ type: 'frame', frame: { type: 'rejected', key: 'k1', code, retryable: false } });
  });

  it('a refused op is reported as rejected with the server code; a network error stops the rest and loses a live connection', async () => {
    const { engine, env, shell } = setup();
    shell.start();
    env.reply = async () => ({ status: 409, body: { error: { code: 'conflict', retryable: false } } });
    engine.effect({ type: 'sendOps', ops: [op('k1')] });
    await settle();
    expect(engine.dispatched.at(-1)).toMatchObject({ type: 'frame', frame: { type: 'rejected', key: 'k1', code: 'conflict', retryable: false } });

    engine.connection = 'live';
    env.reply = async () => {
      throw new Error('network');
    };
    const before = env.posts.length;
    engine.effect({ type: 'sendOps', ops: [op('k2'), op('k3')] });
    await settle();
    expect(env.posts.length - before).toBe(1); // k3 waits: it depends on k2
    expect(engine.dispatched.at(-1)).toEqual({ type: 'disconnected' });
  });

  it('stop clears every timer and closes the socket', () => {
    const { engine, env, shell } = setup();
    shell.start();
    engine.effect({ type: 'subscribe', afterSeq: 0 });
    shell.stop();
    expect(env.timers).toHaveLength(0);
    expect(env.sockets[0]!.closed).toBe(true);
  });

  it('an exception while reporting a refusal does not stop the ops that come later', async () => {
    const { engine, env, shell } = setup();
    shell.start();
    env.reply = async () => ({ status: 409, body: { error: { code: 'conflict', retryable: false } } });
    engine.throwOn = (a) => a.type === 'frame' && a.frame.type === 'rejected';
    engine.effect({ type: 'sendOps', ops: [op('k1')] });
    await settle();
    expect(env.posts).toHaveLength(1);
    engine.throwOn = null;
    engine.effect({ type: 'sendOps', ops: [op('k2')] });
    await settle();
    expect(env.posts.map((p) => (p.body as { ops: { key: string }[] }).ops[0]!.key)).toEqual(['k1', 'k2']);
  });

  it('an error body that is not what the server writes cannot make the report invalid', async () => {
    const { engine, env, shell } = setup();
    shell.start();
    env.reply = async () => ({ status: 502, body: { error: { code: 5, retryable: 'x' } } });
    engine.effect({ type: 'sendOps', ops: [op('k1')] });
    await settle();
    expect(engine.dispatched.at(-1)).toMatchObject({ type: 'frame', frame: { type: 'rejected', key: 'k1', code: 'unavailable', retryable: true } });
    env.reply = async () => ({ status: 502, body: '<html>bad gateway</html>' });
    engine.effect({ type: 'sendOps', ops: [op('k2')] });
    await settle();
    expect(engine.dispatched.at(-1)).toMatchObject({ frame: { type: 'rejected', key: 'k2', code: 'unavailable', retryable: true } });
  });

  it('an event frame keeps the idempotency key the server names, so the app knows its own op', () => {
    const { engine, env, shell } = setup();
    shell.start();
    engine.effect({ type: 'subscribe', afterSeq: 0 });
    env.sockets[0]!.onmessage('{"type":"event","seq":4,"actor":"u","key":"web-1-3","command":{"type":"removeNode","id":"a"}}');
    expect(engine.dispatched.at(-1)).toMatchObject({ type: 'frame', frame: { type: 'event', seq: 4, key: 'web-1-3' } });
  });

  it('a request that fails after the connection was replaced does not lose the new connection', async () => {
    const { engine, env, shell } = setup();
    shell.start();
    engine.connection = 'live';
    let failIt: (e: Error) => void = () => {};
    env.reply = () => new Promise((_, reject) => (failIt = reject));
    engine.effect({ type: 'sendOps', ops: [op('k1')] });
    await settle();
    engine.effect({ type: 'subscribe', afterSeq: 0 }); // the connection is replaced (a reconnect)
    const before = engine.dispatched.length;
    failIt(new Error('timeout'));
    await settle();
    expect(engine.dispatched.slice(before)).toEqual([]);
    expect(env.sockets[0]!.closed).toBe(false);
  });

  it('the socket and its ping are dropped when the app leaves the live state', () => {
    const { engine, env, shell } = setup();
    shell.start();
    engine.effect({ type: 'subscribe', afterSeq: 0 });
    const s = env.sockets[0]!;
    const timersWithPing = env.timers.length;
    engine.moveTo('live');
    engine.moveTo('offline'); // the app found the live connection silent
    expect(s.closed).toBe(true);
    expect(env.timers.length).toBe(timersWithPing - 1);
    const count = engine.dispatched.length;
    s.onmessage('{"type":"heartbeat"}');
    expect(engine.dispatched).toHaveLength(count);
  });
});
