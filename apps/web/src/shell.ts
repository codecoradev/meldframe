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

export class Shell {
  private socket: SocketLike | null = null;
  private timers = new Set<number>();
  private sending: Promise<void> = Promise.resolve();
  private subscribeId = 0;

  constructor(
    private engine: Engine,
    private env: Env,
    private opts: ShellOptions,
  ) {
    engine.onEffects((fx) => fx.forEach((e) => this.run(e)));
  }

  start(): void {
    this.timers.add(this.env.setTimer(() => this.dispatch({ type: 'tick', nowMs: this.env.now() }), TICK_MS, true));
    this.dispatch({ type: 'start', nowMs: this.env.now() });
  }

  stop(): void {
    for (const t of this.timers) this.env.clearTimer(t);
    this.timers.clear();
    const s = this.socket;
    this.socket = null;
    s?.close();
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

  private subscribe(afterSeq: number): void {
    if (this.socket) {
      // After a resync marker the stream waits for a subscribe on the same socket.
      this.socket.send(JSON.stringify({ type: 'subscribe', id: `s${++this.subscribeId}`, room: this.opts.file, sinceSeq: afterSeq }));
      return;
    }
    const socket = this.env.openSocket(`/v1/live/${encodeURIComponent(this.opts.file)}?after=${afterSeq}`);
    this.socket = socket;
    const mine = () => this.socket === socket;
    const ping = this.env.setTimer(() => mine() && socket.send(JSON.stringify({ type: 'ping' })), PING_MS, true);
    this.timers.add(ping);
    socket.onmessage = (data) => mine() && this.frame(data);
    socket.onclose = () => {
      this.env.clearTimer(ping);
      this.timers.delete(ping);
      if (!mine()) return;
      this.socket = null;
      this.dispatch({ type: 'disconnected' });
    };
  }

  private frame(data: string): void {
    const m = JSON.parse(data) as { type: string; seq?: number; actor?: string; command?: never; afterSeq?: number; latestSeq?: number };
    const nowMs = this.env.now();
    switch (m.type) {
      case 'subscribed':
        return this.dispatch({ type: 'connected', nowMs });
      case 'heartbeat':
        return this.dispatch({ type: 'frame', nowMs, frame: { type: 'heartbeat' } });
      case 'event':
        return this.dispatch({ type: 'frame', nowMs, frame: { type: 'event', seq: m.seq!, actor: m.actor!, command: m.command! } });
      case 'resync':
        return this.dispatch({ type: 'frame', nowMs, frame: { type: 'resync', afterSeq: m.afterSeq!, latestSeq: m.latestSeq! } });
      default:
        return; // pong, ephemeral frames and errors need no action
    }
  }

  /** One request per op, in order (SendOps calls queue behind each other). A refusal goes back to the app. */
  private send(ops: OutOp[]): void {
    this.sending = this.sending.then(async () => {
      for (const op of ops) {
        try {
          const path = `/v1/files/${encodeURIComponent(this.opts.file)}/tools/${this.opts.tool ?? 'apply_commands'}`;
          const res = await this.env.post(path, { ops: [{ key: op.key, command: op.command, deps: op.deps }] });
          if (res.status === 200) continue; // the live event confirms it
          const error = (res.body as { error?: { code?: string; retryable?: boolean } } | null)?.error;
          this.dispatch({
            type: 'frame',
            nowMs: this.env.now(),
            frame: { type: 'rejected', key: op.key, code: error?.code ?? 'unavailable', retryable: error?.retryable ?? true },
          });
        } catch {
          // The network is down: the op stays pending and goes out again once the connection is back.
        }
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
