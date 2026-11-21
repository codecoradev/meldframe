// The shell's one handle on the app layer: the WASM `App` behind JSON. It dispatches actions, keeps the
// latest state, nodes and events for the views, and hands effects to whoever carries them out. It holds
// no editing rules of its own (those live in meldframe-app).
import type { Action, AppState, Effect, Event, Node, Variables } from '@meldframe/protocol';
import type { Scene } from '@meldframe/web-canvas/scene';
import init, { initSync, WasmApp } from '@meldframe/wasm';
import type { MeasureFn } from './measurer';
import { type PaintedBox, sceneFromBoxes } from './scene';

export interface Snapshot {
  state: AppState;
  nodes: Node[];
  /** What the canvas draws: the painted boxes of the nodes, from the Rust render step. Read once per change of the document. */
  scene: Scene;
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
  /** The collections and variables of the file; leave out for a file without any. */
  variables?: Variables;
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
  /** How often the state, the nodes and the scene were read (for tests: a quiet tick must cost nothing). */
  readonly stats = { stateReads: 0, nodeReads: 0, sceneReads: 0 };

  private constructor(private app: WasmApp) {
    this.snap = { state: this.readState(), ...this.readDocument(), events: [] };
  }

  static create(o: EngineOptions): Engine {
    const variables = o.variables ? JSON.stringify(o.variables) : null;
    const app = new WasmApp(o.actor, o.keyPrefix ?? 'web', o.session, JSON.stringify(o.nodes), o.seq, variables);
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
      // The scene is painted from the nodes and the mode: a change of the document reads both, and a mode switch
      // (view state, the nodes are the same) reads the scene only.
      const documentChanged = events.some((e) => e.type === 'documentChanged');
      const modeChanged = events.some((e) => e.type === 'modeChanged');
      this.snap = {
        state: this.readState(),
        ...(documentChanged
          ? this.readDocument()
          : { nodes: this.snap.nodes, scene: modeChanged ? this.readScene() : this.snap.scene }),
        events,
      };
      for (const fn of this.listeners) fn(this.snap);
    }
    if (effects.length) this.effectHandler(effects);
    return effects;
  }

  /**
   * Host configuration of the canvas: draw laid-out boxes (hug, fill, auto layout; the Rust layout does the work) or the
   * stored ones. It is not an action and raises no event, so the scene is read again here when the flag changed.
   * The default is off.
   */
  setLayoutOnCanvas(on: boolean): Effect[] {
    return this.hostChanged(JSON.parse(this.app.setLayoutOnCanvas(on)));
  }

  /**
   * Host configuration of the canvas: the text measurer that sizes texts in the laid-out scene (`MeasureFn`, see
   * measurer.ts; the stub measures until a host gives one). It matters while `setLayoutOnCanvas(true)` is in force: then the
   * scene is read again. Setting the same function again measures again, which is how a host answers fonts that finished
   * loading. A value that is not a function throws.
   */
  setMeasurer(measure: MeasureFn): Effect[] {
    return this.hostChanged(JSON.parse(this.app.setMeasurer(measure)));
  }

  /** The scene is not painted from an event here: read it again and tell the views, when the host's change asked for a redraw. */
  private hostChanged(effects: Effect[]): Effect[] {
    if (effects.length) {
      this.snap = { ...this.snap, scene: this.readScene(), events: [] };
      for (const fn of this.listeners) fn(this.snap);
      this.effectHandler(effects);
    }
    return effects;
  }

  private readState(): AppState {
    this.stats.stateReads++;
    return JSON.parse(this.app.stateJson());
  }

  /** The nodes and the scene of the same document, each read once. */
  private readDocument(): { nodes: Node[]; scene: Scene } {
    this.stats.nodeReads++;
    return { nodes: JSON.parse(this.app.nodesJson()), scene: this.readScene() };
  }

  private readScene(): Scene {
    this.stats.sceneReads++;
    const boxes: PaintedBox[] = JSON.parse(this.app.sceneJson());
    return sceneFromBoxes(boxes);
  }
}
