// The shell carries out the effects of the app layer: it opens the live socket, sends ops over REST, and waits
// for the timers the app asks for. Everything it learns goes back as an action. No editing rules here.
import type { Action, Effect, OutOp } from '@meldframe/protocol';
import type { Engine } from './engine';

export interface SocketLike {
  send(data: string): void;
  close(): void;
  onopen: () => void;
  onmessage: (data: string) => void;
  onclose: () => void;
}

/** Everything the shell takes from the platform, so tests can stand in for it. Paths are relative to the server. */
export interface Env {
  now(): number;
  openSocket(path: string): SocketLike;
  post(path: string, body: unknown): Promise<{ status: number; body: unknown }>;
  setTimer(fn: () => void, ms: number, repeat?: boolean): number;
  clearTimer(id: number): void;
}

export interface ShellOptions {
  file: string;
  /** The tool that applies commands; the server's MCP module registers it for every transport. */
  tool?: string;
}

/** How often the app is told the time, so it can notice a silent connection or a stuck attempt. */
export const TICK_MS = 1000;
/** The server drops a client it has not heard from within 45 s; a JSON ping keeps a quiet one alive. */
export const PING_MS = 15_000;

/** Most times a refused `subscribe` is sent again before the connection counts as lost. */
export const MAX_SUBSCRIBE_RETRIES = 3;
const RETRY_FALLBACK_MS = 1000;

type Wire = { type: string; id?: string; seq?: number; actor?: string; command?: unknown; afterSeq?: number; latestSeq?: number; code?: string; retryAfterMs?: number };

/** A frame of the live socket, checked: null when it is not JSON or lacks a field its type needs. */
export function parseFrame(data: string): Wire | null {
  let m: unknown;
  try {
    m = JSON.parse(data);
  } catch {
    return null;
  }
  if (typeof m !== 'object' || m === null || typeof (m as Wire).type !== 'string') return null;
  const w = m as Wire;
  const num = (v: unknown) => typeof v === 'number' && Number.isFinite(v);
  switch (w.type) {
    case 'event':
      return num(w.seq) && typeof w.actor === 'string' && typeof w.command === 'object' && w.command !== null ? w : null;
    case 'resync':
      return num(w.afterSeq) && num(w.latestSeq) ? w : null;
    default:
      return w;
  }
}

export class Shell {
  private socket: SocketLike | null = null;
  private timers = new Set<number>();
  private sending: Promise<void> = Promise.resolve();
  private epoch = 0;
  private subscribeId = 0;
  private lastSub: { id: string; afterSeq: number; tries: number } | null = null;
  private warned = false;

  constructor(
    private engine: Engine,
    private env: Env,
    private opts: ShellOptions,
  ) {
    engine.onEffects((fx) => fx.forEach((e) => this.run(e)));
    // A halted app ignores everything but the UI: nothing left to keep alive until the page is reloaded.
    engine.subscribe((s) => s.state.connection.type === 'halted' && this.stop());
  }

  start(): void {
    this.timers.add(this.env.setTimer(() => this.dispatch({ type: 'tick', nowMs: this.env.now() }), TICK_MS, true));
    this.dispatch({ type: 'start', nowMs: this.env.now() });
  }

  stop(): void {
    for (const t of this.timers) this.env.clearTimer(t);
    this.timers.clear();
    this.epoch++;
    this.drop();
  }

  private dispatch(a: Action): void {
    this.engine.dispatch(a);
  }

  private run(e: Effect): void {
    switch (e.type) {
      case 'subscribe':
        return this.subscribe(e.afterSeq);
      case 'sendOps':
        return this.send(e.ops);
      case 'reconnect':
        return this.later(e.afterMs, { type: 'reconnect', nowMs: 0 });
      case 'wakeAfter':
        return this.later(e.afterMs, { type: 'tick', nowMs: 0 });
      case 'render':
        return; // the views redraw from every snapshot
    }
  }

  private later(ms: number, action: Extract<Action, { nowMs: number }>): void {
    const id = this.env.setTimer(() => {
      this.timers.delete(id);
      this.dispatch({ ...action, nowMs: this.env.now() });
    }, ms);
    this.timers.add(id);
  }

  /** Forget the socket and close it; what it says or does from now on is ignored. */
  private drop(): void {
    const s = this.socket;
    this.socket = null;
    this.lastSub = null;
    try {
      s?.close();
    } catch {
      // already gone
    }
  }

  /** The connection is lost for the app: drop the socket and tell it. */
  private lost(): void {
    this.drop();
    this.dispatch({ type: 'disconnected' });
  }

  private send1(socket: SocketLike, msg: object): boolean {
    try {
      socket.send(JSON.stringify(msg));
      return true;
    } catch {
      return false; // not open (yet or any more)
    }
  }

  private subscribe(afterSeq: number): void {
    const live = this.engine.snapshot().state.connection.type === 'live';
    if (this.socket && live) {
      // A resync marker or a gap: the stream waits for a subscribe on the same socket.
      this.resubscribe(afterSeq, 0);
      return;
    }
    // Connecting or reconnecting: whatever socket there was is not trusted any more.
    this.drop();
    const socket = this.env.openSocket(`/v1/live/${encodeURIComponent(this.opts.file)}?after=${afterSeq}`);
    this.socket = socket;
    const mine = () => this.socket === socket;
    socket.onmessage = (data) => mine() && this.frame(data);
    socket.onclose = () => mine() && this.lost();
    const ping = this.env.setTimer(() => {
      if (mine() && !this.send1(socket, { type: 'ping' })) this.lost();
    }, PING_MS, true);
    this.timers.add(ping);
  }

  private resubscribe(afterSeq: number, tries: number): void {
    const socket = this.socket;
    if (!socket) return;
    const id = `s${++this.subscribeId}`;
    this.lastSub = { id, afterSeq, tries };
    if (!this.send1(socket, { type: 'subscribe', id, room: this.opts.file, sinceSeq: afterSeq })) this.lost();
  }

  private frame(data: string): void {
    const m = parseFrame(data);
    if (!m) return this.lost();
    const nowMs = this.env.now();
    switch (m.type) {
      case 'subscribed':
        return this.dispatch({ type: 'connected', nowMs });
      case 'heartbeat':
        return this.dispatch({ type: 'frame', nowMs, frame: { type: 'heartbeat' } });
      case 'event':
        return this.dispatch({ type: 'frame', nowMs, frame: { type: 'event', seq: m.seq!, actor: m.actor!, command: m.command as never } });
      case 'resync':
        return this.dispatch({ type: 'frame', nowMs, frame: { type: 'resync', afterSeq: m.afterSeq!, latestSeq: m.latestSeq! } });
      case 'error':
        return this.refused(m);
      default:
        return; // pong and ephemeral frames need no action
    }
  }

  /** An `error` frame. Only the answer to our last `subscribe` matters. */
  private refused(m: Wire): void {
    const sub = this.lastSub;
    if (!sub || m.id !== sub.id) return;
    if (m.code === 'rate_limited' || m.code === 'unavailable') {
      if (sub.tries >= MAX_SUBSCRIBE_RETRIES) return this.lost();
      const socket = this.socket;
      const id = this.env.setTimer(() => {
        this.timers.delete(id);
        if (this.socket === socket) this.resubscribe(sub.afterSeq, sub.tries + 1);
      }, m.retryAfterMs ?? RETRY_FALLBACK_MS);
      this.timers.add(id);
      return;
    }
    // bad_request: a subscribe before the marker. The marker is on its way; wait for it.
    if (!this.warned) {
      this.warned = true;
      console.warn('live: subscribe refused, waiting for the resync marker', m.code);
    }
  }

  /** One request per op, in order (SendOps calls queue behind each other). */
  private send(ops: OutOp[]): void {
    const epoch = this.epoch;
    this.sending = this.sending.then(async () => {
      for (const op of ops) {
        if (epoch !== this.epoch) return; // the connection was lost: the app sends everything again
        const path = `/v1/files/${encodeURIComponent(this.opts.file)}/tools/${this.opts.tool ?? 'apply_commands'}`;
        let res: { status: number; body: unknown };
        try {
          res = await this.env.post(path, { ops: [{ key: op.key, command: op.command, deps: op.deps }] });
        } catch {
          // The network failed. Stop here: the later ops depend on this one, and the app sends every
          // unconfirmed op again, in order and with the same keys, when the connection is back.
          this.epoch++;
          this.lost();
          return;
        }
        if (res.status === 200) continue; // the live event confirms it
        const error = (res.body as { error?: { code?: string; retryable?: boolean } } | null)?.error;
        this.dispatch({
          type: 'frame',
          nowMs: this.env.now(),
          frame: { type: 'rejected', key: op.key, code: error?.code ?? 'unavailable', retryable: error?.retryable ?? true },
        });
      }
    });
  }
}

/** The browser's platform: same-origin paths (the dev server proxies `/v1` and adds the token). */
export function browserEnv(): Env {
  return {
    now: () => Date.now(),
    openSocket(path) {
      const url = new URL(path, location.href);
      url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
      const ws = new WebSocket(url);
      const s: SocketLike = { send: (d) => ws.send(d), close: () => ws.close(), onopen() {}, onmessage() {}, onclose() {} };
      ws.onopen = () => s.onopen();
      ws.onmessage = (e) => s.onmessage(String(e.data));
      ws.onclose = () => s.onclose();
      ws.onerror = () => {}; // a close follows
      return s;
    },
    async post(path, body) {
      const res = await fetch(path, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) });
      return { status: res.status, body: await res.json().catch(() => null) };
    },
    setTimer: (fn, ms, repeat) => (repeat ? setInterval(fn, ms) : setTimeout(fn, ms)) as unknown as number,
    clearTimer: (id) => {
      clearTimeout(id);
      clearInterval(id);
    },
  };
}
