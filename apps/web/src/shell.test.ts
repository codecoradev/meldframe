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
  onopen: () => void = () => {};
  onmessage: (data: string) => void = () => {};
  onclose: () => void = () => {};
  constructor(public url: string) {}
  send(s: string) {
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
  reply: () => Promise<{ status: number; body: unknown }> = async () => ({ status: 200, body: { accepted: [] } });
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
  get socket() {
    return this.sockets[this.sockets.length - 1]!;
  }
}

const flush = () => new Promise((r) => setTimeout(r, 0));
const add = (id: string): Command =>
  ({ type: 'addNode', node: { id, name: id, type: 'rect', parentId: null, x: 0, y: 0, w: 10, h: 10 } as Node }) as Command;

function setup() {
  const env = new FakeEnv();
  const engine = Engine.create({ actor: 'usr_me', nodes: [], seq: 0, session: 3 });
  const shell = new Shell(engine, env, { file: 'file_dev' });
  return { env, engine, shell };
}

describe('Shell interprets effects', () => {
  it('opens the live socket from the confirmed number and goes live when subscribed', () => {
    const { env, engine, shell } = setup();
    shell.start();
    expect(env.socket.url).toBe('/v1/live/file_dev?after=0');
    expect(engine.snapshot().state.connection.type).toBe('connecting');
    env.socket.serve({ type: 'subscribed', id: 's', room: 'file_dev' });
    expect(engine.snapshot().state.connection.type).toBe('live');
  });

  it('shows another actor\'s event without a reload, labelled with the actor', () => {
    const { env, engine, shell } = setup();
    shell.start();
    env.socket.serve({ type: 'subscribed', id: 's', room: 'file_dev' });
    env.socket.serve({ type: 'event', room: 'file_dev', seq: 1, actor: 'agent:alpha', command: add('n1') });
    const { nodes, state } = engine.snapshot();
    expect(nodes.map((n) => n.id)).toEqual(['n1']);
    expect(state.lastActor['n1']).toBe('agent:alpha');
    expect(state.confirmedSeq).toBe(1);
  });

  it('sends the person\'s ops over REST in order, one request each, with their keys', async () => {
    const { env, engine, shell } = setup();
    shell.start();
    env.socket.serve({ type: 'subscribed', id: 's', room: 'file_dev' });
    engine.dispatch({ type: 'createNode', kind: 'rectangle', x: 1, y: 2, parent: null });
    await flush();
    expect(env.posts).toHaveLength(1);
    expect(env.posts[0]!.path).toBe('/v1/files/file_dev/tools/apply_commands');
    const op = env.posts[0]!.body.ops[0]!;
    expect(op.key).toMatch(/^web-/);
    expect(op.command.type).toBe('addNode');
  });

  it('reports a refused op back to the app, which rolls the view back', async () => {
    const { env, engine, shell } = setup();
    env.reply = async () => ({ status: 409, body: { error: { code: 'conflict', message: 'm', retryable: false } } });
    shell.start();
    env.socket.serve({ type: 'subscribed', id: 's', room: 'file_dev' });
    engine.dispatch({ type: 'createNode', kind: 'frame', x: 0, y: 0, parent: null });
    await flush();
    expect(engine.snapshot().events.some((e) => e.type === 'opRejected' && e.code === 'conflict')).toBe(true);
    expect(engine.snapshot().nodes).toHaveLength(0);
  });

  it('reconnects with backoff after a drop and subscribes after the last number it has', () => {
    const { env, engine, shell } = setup();
    shell.start();
    env.socket.serve({ type: 'subscribed', id: 's', room: 'file_dev' });
    env.socket.serve({ type: 'event', room: 'file_dev', seq: 1, actor: 'agent:alpha', command: add('n1') });
    env.socket.drop();
    expect(['offline', 'reconnecting']).toContain(engine.snapshot().state.connection.type);
    const before = env.sockets.length;
    env.advance(60_000);
    expect(env.sockets.length).toBeGreaterThan(before);
    expect(env.socket.url).toBe('/v1/live/file_dev?after=1');
    env.socket.serve({ type: 'subscribed', id: 's', room: 'file_dev' });
    expect(engine.snapshot().state.connection.type).toBe('live');
  });

  it('answers a resync marker by subscribing again on the same socket', () => {
    const { env, shell } = setup();
    shell.start();
    const socket = env.socket;
    socket.serve({ type: 'subscribed', id: 's', room: 'file_dev' });
    socket.serve({ type: 'resync', room: 'file_dev', afterSeq: 0, latestSeq: 5 });
    const sub = socket.sent.map((s) => JSON.parse(s)).find((m) => m.type === 'subscribe');
    expect(sub).toMatchObject({ room: 'file_dev', sinceSeq: 0 });
    expect(env.sockets).toHaveLength(1);
  });

  it('pings so the server does not reap a quiet connection', () => {
    const { env, shell } = setup();
    shell.start();
    env.socket.serve({ type: 'subscribed', id: 's', room: 'file_dev' });
    env.advance(30_000);
    expect(env.socket.sent.filter((s) => JSON.parse(s).type === 'ping').length).toBeGreaterThanOrEqual(1);
  });

  it('stops: closes the socket and the timers', () => {
    const { env, shell } = setup();
    shell.start();
    shell.stop();
    expect(env.socket.closed).toBe(true);
    expect(env.timers).toEqual([]);
  });
});
