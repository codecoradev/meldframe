// The shell's one handle on the app layer: the WASM `App` behind JSON. It dispatches actions, keeps the
// latest state, nodes and events for the views, and hands effects to whoever carries them out. It holds
// no editing rules of its own (those live in meldframe-app).
import type { Action, AppState, Effect, Event, Node } from '@meldframe/protocol';
import init, { initSync, WasmApp } from './wasm/meldframe_wasm.js';

export interface Snapshot {
  state: AppState;
  nodes: Node[];
  /** The events of the last dispatch, oldest first. */
  events: Event[];
}

export interface EngineOptions {
  actor: string;
  /** Stable per client kind; with `session` it makes the idempotency keys. */
  keyPrefix?: string;
  /** A fresh random number per page load (see `App::new`). */
  session: number;
  nodes: Node[];
  seq: number;
}

/** Loads the WASM module once. Give the bytes in Node; in the browser it fetches the module itself. */
export async function loadWasm(bytes?: BufferSource): Promise<void> {
  if (bytes) initSync({ module: bytes });
  else await init();
}

export class Engine {
  private listeners = new Set<(s: Snapshot) => void>();
  private effectHandler: (fx: Effect[]) => void = () => {};
  private snap: Snapshot;
  /** How often the state and the nodes were parsed (for tests: a quiet tick must cost nothing). */
  readonly stats = { stateReads: 0, nodeReads: 0 };

  private constructor(private app: WasmApp) {
    this.snap = { state: this.readState(), nodes: this.readNodes(), events: [] };
  }

  static create(o: EngineOptions): Engine {
    const app = new WasmApp(o.actor, o.keyPrefix ?? 'web', o.session, JSON.stringify(o.nodes), o.seq);
    return new Engine(app);
  }

  snapshot(): Snapshot {
    return this.snap;
  }

  subscribe(fn: (s: Snapshot) => void): () => void {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  }

  /** The one handler of effects (the transport); a later call replaces it. */
  onEffects(fn: (fx: Effect[]) => void): void {
    this.effectHandler = fn;
  }

  dispatch(action: Action): Effect[] {
    const effects: Effect[] = JSON.parse(this.app.dispatch(JSON.stringify(action)));
    const events: Event[] = JSON.parse(this.app.takeEventsJson());
    // Every change of state or document raises an event; a dispatch without events (a quiet tick, a
    // heartbeat) leaves the snapshot as it is, so the views have nothing to redo.
    if (events.length) {
      const changed = events.some((e) => e.type === 'documentChanged');
      this.snap = {
        state: this.readState(),
        nodes: changed ? this.readNodes() : this.snap.nodes,
        events,
      };
      for (const fn of this.listeners) fn(this.snap);
    }
    if (effects.length) this.effectHandler(effects);
    return effects;
  }

  private readState(): AppState {
    this.stats.stateReads++;
    return JSON.parse(this.app.stateJson());
  }

  private readNodes(): Node[] {
    this.stats.nodeReads++;
    return JSON.parse(this.app.nodesJson());
  }
}
