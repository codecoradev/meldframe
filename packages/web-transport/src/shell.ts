// The shell carries out the effects of the app layer: it opens the live socket, sends ops over REST, and waits
// for the timers the app asks for. Everything it learns goes back as an action. No editing rules here.
import type { Action, AppState, Effect, OutOp, ServerMsg, Wire } from '@meldframe/protocol';
import { assetsQuery } from './assets';
import { listQuery } from './batches';
import { checkpointCall, type CheckpointCall } from './checkpoints';

/** An agent asks this editor for a PNG of a node (or of the whole page when `nodeId` is absent). */
export type CaptureRequest = Extract<ServerMsg, { type: 'captureRequest' }>;

/** What the shell needs of the app layer. `Engine` in engine.ts (the WASM app behind JSON) is one; tests bring another. */
export interface Engine {
  snapshot(): { state: AppState };
  subscribe(fn: (s: { state: AppState }) => void): () => void;
  /** The one handler of effects; a later call replaces it. */
  onEffects(fn: (fx: Effect[]) => void): void;
  dispatch(action: Action): Effect[];
}

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
  /** A POST that rejects after `timeoutMs` (a hung request must not hold up the ops behind it). The answer is the status and the text of the body, not read: the app layer reads the refusal of an op (its code, whether it may be sent again). */
  post(path: string, body: unknown, timeoutMs?: number): Promise<{ status: number; text: string }>;
  /** A request of the routes the app layer reads the answer of (batches, checkpoints, assets); rejects after `timeoutMs`. The answer is the status and the text of the body, not read: the app layer parses and checks it. */
  request(method: 'GET' | 'PUT' | 'POST', path: string, body?: unknown, timeoutMs?: number): Promise<{ status: number; text: string }>;
  setTimer(fn: () => void, ms: number, repeat?: boolean): number;
  clearTimer(id: number): void;
}

export interface ShellOptions {
  file: string;
  /** The tool that applies commands; the server's MCP module registers it for every transport. */
  tool?: string;
  /**
   * An agent asks for a PNG. With this set, the shell tells the server after each subscribe that this client can capture,
   * and then every request must be answered: by the upload of the image, or by `captureFailed`.
   */
  onCapture?: (r: CaptureRequest) => void;
}

/** How often the app is told the time, so it can notice a silent connection or a stuck attempt. */
export const TICK_MS = 1000;
/** The server drops a client it has not heard from within 45 s; a JSON ping keeps a quiet one alive. */
export const PING_MS = 15_000;
/** How long a REST call may take before it counts as a network error. */
export const POST_TIMEOUT_MS = 15_000;

type Msg = { type: string; id?: string; seq?: number; actor?: string; nodeIds?: unknown; key?: unknown; requestId?: unknown; scale?: unknown; maxPixels?: unknown; minSeq?: unknown; uploadToken?: unknown; timeoutMs?: unknown; nodeId?: unknown; command?: unknown; afterSeq?: number; latestSeq?: number };

/** A frame of the live socket, checked: null when it is not JSON or lacks a field its type needs. */
export function parseFrame(data: string): Msg | null {
  let m: unknown;
  try {
    m = JSON.parse(data);
  } catch {
    return null;
  }
  if (typeof m !== 'object' || m === null || typeof (m as Msg).type !== 'string') return null;
  const w = m as Msg;
  const num = (v: unknown) => typeof v === 'number' && Number.isInteger(v) && v >= 0;
  switch (w.type) {
    case 'event':
      return num(w.seq) && typeof w.actor === 'string' && typeof w.command === 'object' && w.command !== null ? w : null;
    case 'resync':
      return num(w.afterSeq) && num(w.latestSeq) ? w : null;
    case 'captureRequest':
      return typeof w.requestId === 'string' && typeof w.uploadToken === 'string' && typeof w.scale === 'number' && typeof w.maxPixels === 'number' &&
        num(w.minSeq) && num(w.timeoutMs) && (w.nodeId === undefined || typeof w.nodeId === 'string')
        ? w
        : null;
    default:
      return w;
  }
}

export class Shell {
  private socket: SocketLike | null = null;
  private timers = new Set<number>();
  private sending: Promise<void> = Promise.resolve();
  private epoch = 0;
  /** Counts stops: an answer of a request made before a stop is dropped. */
  private stopped = 0;
  private subscribeId = 0;
  /** The id of the last `subscribe` sent on the socket: an `error` frame answers it, or it is stale. */
  private lastSub: string | null = null;
  private warnedAnswer = false;
  private ping: number | null = null;

  constructor(
    private engine: Engine,
    private env: Env,
    private opts: ShellOptions,
  ) {
    engine.onEffects((fx) => fx.forEach((e) => this.run(e)));
    // A halted app ignores everything but the UI: nothing left to keep alive until the page is reloaded.
    let before = engine.snapshot().state.connection.type;
    engine.subscribe((s) => {
      const type = s.state.connection.type;
      if (type === 'halted') this.stop();
      // A live connection that the app found silent: nothing it says is wanted, and a socket kept until the next
      // `subscribe` effect would go on pinging and reporting frames meanwhile. (An attempt that got stuck keeps its
      // socket: a late answer on it is still taken, and the next attempt replaces it.)
      else if (type === 'offline' && before === 'live') this.drop();
      before = type;
    });
  }

  start(): void {
    this.timers.add(this.env.setTimer(() => this.dispatch({ type: 'tick', nowMs: this.env.now() }), TICK_MS, true));
    this.dispatch({ type: 'start', nowMs: this.env.now() });
  }

  stop(): void {
    for (const t of this.timers) this.env.clearTimer(t);
    this.timers.clear();
    this.epoch++;
    this.stopped++;
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
      case 'fetchBatches':
        return this.batches('GET', this.batchPath(listQuery(e)), undefined, (status, body) => ({ kind: 'batches', status, body }));
      case 'putBatchReview':
        return this.batches('PUT', this.batchPath(`/${Math.trunc(e.n)}/review`), { state: e.state }, (status, body) => ({ kind: 'batchReview', status, body }));
      case 'fetchAssets':
        return this.fetching('GET', `/v1/assets${assetsQuery(e)}`, undefined, (status, body) => ({ kind: 'assets', request: e.request, after: e.after, status, body }), { type: 'assetsRequestFailed', request: e.request, code: 'unavailable' });
      case 'fetchCheckpoints':
      case 'postCheckpoint':
      case 'fetchCheckpointState':
        return this.checkpoint(checkpointCall(this.opts.file, e));
      case 'render':
        return; // the views redraw from every snapshot
      case 'raster':
      case 'finishRaster':
      case 'failCapture':
        return; // the legacy client never starts these: it never dispatches `png` or sends the wires they answer
      default: {
        const unhandled: never = e; // a new effect must be carried out here, or this stops compiling
        return unhandled;
      }
    }
  }

  private batchPath(tail: string): string {
    return `/v1/files/${encodeURIComponent(this.opts.file)}/batches${tail}`;
  }

  /** One request of the batch routes; whatever happens, the app is told once (a failure is `batchRequestFailed`). */
  private batches(method: 'GET' | 'PUT', path: string, body: unknown, wire: (status: number, text: string) => Wire): void {
    this.fetching(method, path, body, wire, { type: 'batchRequestFailed', code: 'unavailable' });
  }

  /** One request that is not an op (the batch routes, the asset list): the app is told once, with the raw answer, or by `failed` when the network fails. */
  private fetching(method: 'GET' | 'PUT', path: string, body: unknown, wire: (status: number, text: string) => Wire, failed: Action): void {
    const gen = this.stopped;
    void this.env.request(method, path, body, POST_TIMEOUT_MS).then(
      (r) => this.answered(gen, { type: 'wire', wire: wire(r.status, r.text) }),
      () => this.answered(gen, failed),
    );
  }

  /**
   * One request of the checkpoint routes (ADR-0023); whatever happens, the app is told once, and a failure names its
   * request. An answer the app cannot take is told to it as the failure of the request, so a read or a revert that waits
   * for it is never left waiting: the deadline of a request is the client timeout of `Env`, as the app has no clock here.
   */
  private checkpoint(c: CheckpointCall): void {
    const gen = this.stopped;
    void this.env.request(c.method, c.path, c.body, POST_TIMEOUT_MS).then(
      (r) => this.answered(gen, { type: 'wire', wire: c.wire(r.status, r.text) }, c.failure),
      () => this.answered(gen, c.failure),
    );
  }

  private answered(gen: number, a: Action, failure?: Action): void {
    if (gen !== this.stopped) return; // the shell was stopped meanwhile
    try {
      this.dispatch(a);
    } catch (err) {
      if (failure) this.failed(failure);
      // The app took no part of it; the panel heals by the next frame or fetch. Say so once: a wasm panic must not stay silent.
      if (!this.warnedAnswer) {
        this.warnedAnswer = true;
        console.warn('batches: the app could not take an answer', err);
      }
    }
  }

  /** The failure of a request, told once more when the answer was not taken: if the app cannot take this either, nothing is left to do. */
  private failed(failure: Action): void {
    try {
      this.dispatch(failure);
    } catch {
      // the app is past help; the next frame or fetch heals what it can
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
    // Requests queued or in flight belong to the connection that is gone: the app sends everything again, with
    // the same keys, once it is back. A failure of such a request must not count against the next connection.
    this.epoch++;
    if (this.ping !== null) {
      this.env.clearTimer(this.ping);
      this.timers.delete(this.ping);
      this.ping = null;
    }
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
      this.resubscribe(afterSeq);
      return;
    }
    // Connecting or reconnecting: whatever socket there was is not trusted any more.
    this.drop();
    const socket = this.env.openSocket(`/v1/live/${encodeURIComponent(this.opts.file)}?after=${afterSeq}`);
    this.socket = socket;
    const mine = () => this.socket === socket;
    socket.onmessage = (data) => mine() && this.frame(data);
    socket.onclose = () => mine() && this.lost();
    this.ping = this.env.setTimer(() => {
      if (mine() && !this.send1(socket, { type: 'ping' })) this.lost();
    }, PING_MS, true);
    this.timers.add(this.ping);
  }

  private resubscribe(afterSeq: number): void {
    const socket = this.socket;
    if (!socket) return;
    const id = `s${++this.subscribeId}`;
    this.lastSub = id;
    if (!this.send1(socket, { type: 'subscribe', id, room: this.opts.file, sinceSeq: afterSeq })) this.lost();
  }

  private frame(data: string): void {
    const m = parseFrame(data);
    if (!m) return this.lost();
    try {
      this.apply(m, data);
    } catch {
      // A frame the app cannot take (it passed the shape check but not the app's own): the view would go
      // stale while heartbeats look alive, so start again from the last confirmed number.
      this.lost();
    }
  }

  private apply(m: Msg, data: string): void {
    const nowMs = this.env.now();
    switch (m.type) {
      case 'subscribed':
        this.dispatch({ type: 'connected', nowMs });
        if (this.opts.onCapture && this.socket) this.send1(this.socket, { type: 'capabilities', room: this.opts.file, capabilities: ['capture'] });
        return;
      case 'captureRequest':
        try {
          this.opts.onCapture?.(m as unknown as CaptureRequest);
        } catch {
          // a view that fails on a request must not cost the connection; the agent's request times out on the server
        }
        return;
      case 'heartbeat':
        return this.dispatch({ type: 'frame', nowMs, frame: { type: 'heartbeat' } });
      case 'event':
        // `key` is the idempotency key of the op: with it the app knows an event is its own op, not just an equal one.
        return this.dispatch({
          type: 'frame',
          nowMs,
          frame: { type: 'event', seq: m.seq!, actor: m.actor!, ...(typeof m.key === 'string' ? { key: m.key } : {}), command: m.command as never },
        });
      case 'resync':
        return this.dispatch({ type: 'frame', nowMs, frame: { type: 'resync', afterSeq: m.afterSeq!, latestSeq: m.latestSeq! } });
      case 'batch':
      case 'touch':
      case 'checkpoint':
        // An ephemeral hint over the log, handed to the app layer as the text the socket sent: it checks the frame and ignores
        // one that does not pass (a fetch heals the gap). One it cannot take goes to `lost()` (a reconnect) through `frame()`, like any frame.
        return this.dispatch({ type: 'wire', wire: { kind: 'hint', nowMs, text: data } });
      case 'error':
        return this.refused(m.id, data);
      default:
        return; // pong and ephemeral frames need no action
    }
  }

  /** Tells the server this editor cannot serve a capture request (codes: `node_not_found`, `too_large`, `stale_client`, ...). */
  captureFailed(requestId: string, code: string, message: string): void {
    if (this.socket) this.send1(this.socket, { type: 'captureFailed', requestId, code, message });
  }

  /**
   * An `error` frame. Only the answer to our last `subscribe` matters: the ids are the shell's, so the shell matches
   * them. The app layer reads the frame and answers with a new subscribe (after the wait), with nothing (it waits for the
   * resync marker) or by going offline; the count, the wait and the default are its rules (`meldframe_app_rules::subscribe`).
   */
  private refused(id: string | undefined, data: string): void {
    if (this.lastSub === null || id !== this.lastSub) return;
    this.dispatch({ type: 'wire', wire: { kind: 'subscribeRefused', nowMs: this.env.now(), text: data } });
  }

  /** One request per op, in order (SendOps calls queue behind each other). */
  private send(ops: OutOp[]): void {
    const epoch = this.epoch;
    const run = async () => {
      try {
        await this.post(ops, epoch);
      } catch {
        // Whatever threw (the app refusing an action, a trap, a listener): the queue must go on, or no op would
        // ever be sent again. The ops of this call are unconfirmed; the app sends them again after a new connection.
        if (epoch === this.epoch) this.lost();
      }
    };
    this.sending = this.sending.then(run);
  }

  private async post(ops: OutOp[], epoch: number): Promise<void> {
    for (const op of ops) {
      if (epoch !== this.epoch) return; // the connection was lost: the app sends everything again
      const path = `/v1/files/${encodeURIComponent(this.opts.file)}/tools/${this.opts.tool ?? 'apply_commands'}`;
      let res: { status: number; text: string };
      try {
        res = await this.env.post(path, { ops: [{ key: op.key, command: op.command, deps: op.deps }] }, POST_TIMEOUT_MS);
      } catch {
        // The network failed. Stop here: the later ops depend on this one, and the app sends every unconfirmed op
        // again, in order and with the same keys, when the connection is back. A request of a connection that was
        // replaced meanwhile says nothing about the new one.
        if (epoch !== this.epoch) return;
        // Only a live connection is lost by this; one the app already knows is down is on its way back.
        if (this.engine.snapshot().state.connection.type === 'live') this.lost();
        else this.epoch++;
        return;
      }
      if (res.status === 200) continue; // the live event confirms it
      // Anything else is a refusal. The app layer reads the body (it may come from a proxy or a gateway): the code, whether
      // the op may be sent again, and that a body that is not the server's means "for now, try again".
      this.dispatch({ type: 'wire', wire: { kind: 'opRefused', nowMs: this.env.now(), key: op.key, status: res.status, body: res.text } });
    }
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
    async post(path, body, timeoutMs) {
      const res = await fetch(path, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(body),
        signal: timeoutMs ? AbortSignal.timeout(timeoutMs) : undefined,
      });
      // The text of the body, not read (as `request` does): a body that is not text, or fails halfway, is an empty one.
      return { status: res.status, text: await res.text().catch(() => '') };
    },
    async request(method, path, body, timeoutMs) {
      const res = await fetch(path, {
        method,
        headers: body === undefined ? undefined : { 'content-type': 'application/json' },
        body: body === undefined ? undefined : JSON.stringify(body),
        signal: timeoutMs ? AbortSignal.timeout(timeoutMs) : undefined,
      });
      // The text of the body, not read: a body that is not text (or that fails halfway) is an empty one, which the app layer does not take for an answer.
      return { status: res.status, text: await res.text().catch(() => '') };
    },
    setTimer: (fn, ms, repeat) => (repeat ? setInterval(fn, ms) : setTimeout(fn, ms)) as unknown as number,
    clearTimer: (id) => {
      clearTimeout(id);
      clearInterval(id);
    },
  };
}
