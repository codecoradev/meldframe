import { readFileSync } from 'node:fs';
import type { Command, Node } from '@meldframe/protocol';
import { beforeAll, describe, expect, it } from 'vitest';
import { Engine, loadWasm } from './engine';
import { Shell, type Env, type SocketLike } from './shell';

beforeAll(async () => {
  await loadWasm(readFileSync(new URL('./wasm/meldframe_wasm_bg.wasm', import.meta.url)));
});

class FakeSocket implements SocketLike {
  sent: string[] = [];
  closed = false;
  throwOnSend = false;
  onopen: () => void = () => {};
  onmessage: (data: string) => void = () => {};
  onclose: () => void = () => {};
  constructor(public url: string) {}
  send(s: string) {
    if (this.throwOnSend) throw new Error('socket is not open');
    this.sent.push(s);
  }
  close() {
    this.closed = true;
  }
  /** What the server does. */
  serve(msg: object) {
    this.onmessage(JSON.stringify(msg));
  }
  drop() {
    this.onclose();
  }
}

class FakeEnv implements Env {
  clock = 1_000;
  sockets: FakeSocket[] = [];
  posts: { path: string; body: { ops: { key: string; command: Command }[] }; token?: string }[] = [];
  reply: (n: number) => Promise<{ status: number; body: unknown }> = async () => ({ status: 200, body: { accepted: [] } });
  timers: { at: number; fn: () => void; every?: number; id: number }[] = [];
  private next = 1;
  now = () => this.clock;
  openSocket = (path: string) => {
    const s = new FakeSocket(path);
    this.sockets.push(s);
    return s;
  };
  post = async (path: string, body: { ops: { key: string; command: Command }[] }) => {
    this.posts.push({ path, body });
    return this.reply(this.posts.length);
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
  get socket() {
    return this.sockets[this.sockets.length - 1]!;
  }
}

const flush = () => new Promise((r) => setTimeout(r, 0));
const add = (id: string): Command =>
  ({ type: 'addNode', node: { id, name: id, type: 'rect', parentId: null, x: 0, y: 0, w: 10, h: 10 } as Node }) as Command;
const hello = { type: 'subscribed', id: 's', room: 'file_dev' };
const ev = (seq: number, id: string, actor = 'agent:alpha') => ({ type: 'event', room: 'file_dev', seq, actor, command: add(id) });
const kinds = (e: Engine) => {
  const seen: string[] = [e.snapshot().state.connection.type];
  e.subscribe((s) => {
    const c = s.state.connection.type;
    if (seen[seen.length - 1] !== c) seen.push(c);
  });
  return seen;
};

function setup() {
  const env = new FakeEnv();
  const engine = Engine.create({ actor: 'usr_me', nodes: [], seq: 0, session: 3 });
  const shell = new Shell(engine, env, { file: 'file_dev' });
  return { env, engine, shell, seen: kinds(engine) };
}
const live = () => {
  const c = setup();
  c.shell.start();
  c.env.socket.serve(hello);
  return c;
};
const sentTypes = (s: FakeSocket) => s.sent.map((x) => JSON.parse(x).type);
const create = (e: Engine) => e.dispatch({ type: 'createNode', kind: 'rectangle', x: 1, y: 2, parent: null });

describe('Shell: the connection', () => {
  it('opens the socket from the confirmed number and goes connecting, live', () => {
    const { env, seen, shell } = setup();
    shell.start();
    expect(env.socket.url).toBe('/v1/live/file_dev?after=0');
    env.socket.serve(hello);
    expect(seen).toEqual(['connecting', 'live']);
  });

  it("shows another actor's event without a reload, labelled with the actor", () => {
    const { env, engine } = live();
    env.socket.serve(ev(1, 'n1'));
    const { nodes, state } = engine.snapshot();
    expect(nodes.map((n) => n.id)).toEqual(['n1']);
    expect(state.lastActor['n1']).toBe('agent:alpha');
    expect(state.confirmedSeq).toBe(1);
  });

  it('reconnects after a close: offline, reconnecting, live, with the last number', () => {
    const { env, seen } = live();
    env.socket.serve(ev(1, 'n1'));
    env.socket.drop();
    env.advance(999);
    expect(env.sockets).toHaveLength(1); // the backoff of the first attempt is 1 s
    env.advance(1);
    expect(env.sockets).toHaveLength(2);
    expect(env.socket.url).toBe('/v1/live/file_dev?after=1');
    env.socket.serve(hello);
    expect(seen).toEqual(['connecting', 'live', 'offline', 'reconnecting', 'live']);
  });

  it('replaces a stale socket that never reports a close when the app reconnects', () => {
    const { env, seen } = live();
    const stale = env.socket; // half open: it goes silent and never fires onclose
    env.advance(120_000);
    expect(seen.slice(0, 4)).toEqual(['connecting', 'live', 'offline', 'reconnecting']);
    expect(env.sockets.length).toBeGreaterThanOrEqual(2);
    expect(env.socket).not.toBe(stale);
    expect(stale.closed).toBe(true);
    expect(sentTypes(env.socket)).not.toContain('subscribe'); // a new socket, not a write to the dead one
  });

  it('ignores what a replaced socket says later', () => {
    const { env, engine } = live();
    const stale = env.socket;
    env.advance(120_000);
    env.socket.serve(hello);
    stale.serve(ev(1, 'ghost'));
    stale.drop();
    expect(engine.snapshot().nodes).toEqual([]);
    expect(engine.snapshot().state.connection.type).toBe('live');
  });

  it('answers a resync marker with a subscribe on the same socket from the marker\'s number', () => {
    const { env } = live();
    const socket = env.socket;
    for (let i = 1; i <= 7; i++) socket.serve(ev(i, `n${i}`));
    socket.serve({ type: 'resync', room: 'file_dev', afterSeq: 7, latestSeq: 9 });
    const subs = socket.sent.map((x) => JSON.parse(x)).filter((m) => m.type === 'subscribe');
    expect(subs).toHaveLength(1);
    expect(subs[0]).toMatchObject({ room: 'file_dev', sinceSeq: 7 });
    expect(env.sockets).toHaveLength(1);
  });

  it('pings exactly every 15 s while the connection is quiet but alive', () => {
    const { env } = live();
    const socket = env.socket;
    for (let i = 0; i < 9; i++) {
      env.advance(5_000);
      socket.serve({ type: 'heartbeat' });
    }
    expect(sentTypes(socket).filter((t) => t === 'ping')).toHaveLength(3);
  });

  it('does not throw when the socket refuses a send; it counts as a lost connection', () => {
    const { env, seen } = live();
    env.socket.throwOnSend = true;
    env.advance(15_000);
    expect(seen).toContain('offline');
  });

  it('stops: closes the socket and every timer', () => {
    const { env, shell } = live();
    shell.stop();
    expect(env.socket.closed).toBe(true);
    expect(env.timers).toEqual([]);
  });

  it('stops its timers and closes the socket when the app halts', () => {
    const { env, engine, seen } = live();
    env.socket.serve(ev(1, 'n1'));
    env.socket.serve(ev(2, 'n1')); // the same node added twice: the file and the events no longer fit
    expect(engine.snapshot().state.connection.type).toBe('halted');
    expect(seen).toContain('halted');
    expect(env.timers).toEqual([]);
    expect(env.socket.closed).toBe(true);
  });
});

describe('Shell: ops over REST', () => {
  it("sends the person's ops one request each, in order, with their keys", async () => {
    const { env, engine } = live();
    create(engine);
    await flush();
    expect(env.posts).toHaveLength(1);
    expect(env.posts[0]!.path).toBe('/v1/files/file_dev/tools/apply_commands');
    const op = env.posts[0]!.body.ops[0]!;
    expect(op.key).toMatch(/^web-/);
    expect(op.command.type).toBe('addNode');
  });

  it('reports a refusal back to the app, which rolls the view back', async () => {
    const { env, engine } = setup();
    env.reply = async () => ({ status: 409, body: { error: { code: 'conflict', message: 'm', retryable: false } } });
    engine.constructor; // (the shell below owns the effect handler)
    const shell = new Shell(engine, env, { file: 'file_dev' });
    shell.start();
    env.socket.serve(hello);
    engine.dispatch({ type: 'createNode', kind: 'frame', x: 0, y: 0, parent: null });
    await flush();
    expect(engine.snapshot().events.some((e) => e.type === 'opRejected' && e.code === 'conflict')).toBe(true);
    expect(engine.snapshot().nodes).toHaveLength(0);
  });

  it('stops the batch at a network error: later ops are not sent, the connection counts as lost, and all go out again in order', async () => {
    const { env, engine, seen } = live();
    env.reply = async (n) => {
      if (n === 1) throw new Error('network down');
      return { status: 200, body: { accepted: [] } };
    };
    create(engine);
    create(engine);
    create(engine);
    await flush();
    await flush();
    const first = env.posts.map((p) => p.body.ops[0]!.key);
    expect(first).toHaveLength(1); // the batch stopped at the failure
    expect(seen).toContain('offline');
    const [k1, k2, k3] = engine.snapshot().state.pending;
    expect(first[0]).toBe(k1);
    // The connection comes back: every unconfirmed op is sent again, oldest first, with the same keys.
    env.advance(60_000);
    env.socket.serve(hello);
    await flush();
    await flush();
    await flush();
    const again = env.posts.slice(1).map((p) => p.body.ops[0]!.key);
    expect(again).toEqual([k1, k2, k3]);
  });

  it('does not take an engine exception for a network error', async () => {
    const { env, engine } = live();
    env.reply = async () => ({ status: 200, body: { accepted: [] } });
    create(engine);
    await flush();
    expect(engine.snapshot().state.connection.type).toBe('live');
  });
});

describe('Shell: error and malformed frames', () => {
  const errorFrame = (over: object) => ({ type: 'error', id: 's1', message: 'm', retryable: true, ...over });

  it('retries a refused subscribe after retryAfterMs, a bounded number of times, then drops the connection', () => {
    const { env, seen } = live();
    const socket = env.socket;
    socket.serve({ type: 'resync', room: 'file_dev', afterSeq: 0, latestSeq: 3 });
    const subs = () => sentTypes(socket).filter((t) => t === 'subscribe').length;
    expect(subs()).toBe(1);
    for (let i = 1; i <= 3; i++) {
      socket.serve(errorFrame({ id: `s${i}`, code: 'rate_limited', retryAfterMs: 2_000 }));
      env.advance(1_999);
      expect(subs()).toBe(i);
      env.advance(1);
      expect(subs()).toBe(i + 1);
    }
    socket.serve(errorFrame({ id: 's4', code: 'rate_limited', retryAfterMs: 2_000 }));
    expect(seen).toContain('offline');
  });

  it('waits for the marker after a premature resubscribe (bad_request) and logs it once', () => {
    const { env, seen } = live();
    const warn: unknown[] = [];
    const orig = console.warn;
    console.warn = (...a: unknown[]) => void warn.push(a);
    try {
      env.socket.serve({ type: 'resync', room: 'file_dev', afterSeq: 0, latestSeq: 3 });
      env.socket.serve(errorFrame({ id: 's1', code: 'bad_request', retryable: false }));
      env.socket.serve(errorFrame({ id: 's1', code: 'bad_request', retryable: false }));
    } finally {
      console.warn = orig;
    }
    expect(warn).toHaveLength(1);
    expect(seen).toEqual(['connecting', 'live']);
  });

  it('closes the socket and reconnects on a frame that is not JSON or lacks its fields', () => {
    for (const bad of ['not json', '{"type":"event","seq":"x"}', '{"type":"resync"}', 'null']) {
      const { env, seen } = live();
      const socket = env.socket;
      socket.onmessage(bad);
      expect(socket.closed).toBe(true);
      expect(seen).toContain('offline');
      env.advance(1_000);
      expect(env.sockets.length).toBe(2);
    }
  });
});
