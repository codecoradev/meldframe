// Fakes for the driver tests (driver.test.ts, driver.view.test.ts): a Pixi application that only counts, a host element that keeps its
// listeners, and a `setup` that mounts a driver on them with a clock the test turns by hand. Not product code.
import { vi } from 'vitest';
import { Container } from 'pixi.js';
import { HOME } from '@meldframe/web-canvas/camera';
import type { Scene } from '@meldframe/web-canvas/scene';
import type { CanvasEvents, CanvasProps, HostWatchers, Platform } from './api';
import { CanvasDriver } from './driver';

export const rect = (id: string, x: number, y = 0) => ({ id, parent: null, kind: 'rect' as const, x, y, w: 100, h: 100, fill: 0x808080 });
export const scene = (...nodes: Scene['nodes']): Scene => ({ space: 'parent', nodes });
export const props = (over: Partial<CanvasProps> = {}): CanvasProps => ({
  scene: scene(), selection: [], camera: { ...HOME }, touches: [], indicator: null, tool: 'move', theme: 'dark', ...over,
});

export function fakeApp(failWith?: Error) {
  const stage = new Container();
  const renderer = { name: 'fake', resolution: 1, background: { color: -1 }, render: vi.fn(), resize: vi.fn() };
  return {
    stage, renderer, ticker: { stop: vi.fn() }, destroy: vi.fn(),
    init: vi.fn(async () => {
      if (failWith) throw failWith;
    }),
  };
}
export type FakeApp = ReturnType<typeof fakeApp>;

/** An element that keeps its listeners (the `signal` of `addEventListener` removes them, as in a browser). */
export function fakeHost() {
  const listeners = new Map<string, Set<(e: unknown) => void>>();
  return {
    addEventListener(type: string, fn: (e: unknown) => void, opts?: { signal?: AbortSignal }) {
      const set = listeners.get(type) ?? new Set();
      listeners.set(type, set);
      set.add(fn);
      opts?.signal?.addEventListener('abort', () => set.delete(fn));
    },
    getBoundingClientRect: () => ({ left: 0, top: 0, width: 800, height: 600 }),
    focus: vi.fn(),
    setPointerCapture: vi.fn(),
    style: { cursor: '' },
    count: () => [...listeners.values()].reduce((n, s) => n + s.size, 0),
    emit(type: string, e: object) {
      for (const fn of [...(listeners.get(type) ?? [])]) fn({ preventDefault() {}, ...e });
    },
  };
}
export type FakeHost = ReturnType<typeof fakeHost>;
export const asHost = (h: FakeHost) => h as unknown as HTMLElement;
export const surface = {} as HTMLCanvasElement;

export function setup(app: FakeApp = fakeApp()) {
  const frames = new Map<number, () => void>();
  let nextFrame = 1;
  let watchers: HostWatchers | null = null;
  let unwatched = 0;
  const platform: Platform = {
    requestFrame: (cb) => (frames.set(nextFrame, cb), nextFrame++),
    cancelFrame: (id) => void frames.delete(id),
    dpr: () => 1,
    measure: () => ({ w: 800, h: 600 }),
    watch: (_host, w) => {
      watchers = w;
      w.reducedMotion(false);
      return () => {
        watchers = null;
        unwatched++;
      };
    },
    createApp: () => app as never,
    assets: { fetch: async () => ({ ok: false, status: 404, blob: async () => new Blob(), json: async () => ({}) }), decode: async () => ({ width: 1, height: 1 }) },
  };
  const driver = new CanvasDriver(platform);
  const events = { onReady: vi.fn(), onError: vi.fn(), onCameraChange: vi.fn(), onStatus: vi.fn() } satisfies CanvasEvents;
  driver.events = events;
  const host = fakeHost();
  return {
    app, driver, events, host, platform,
    /** Runs the frames that were asked for (one animation frame). */
    frame() {
      const run = [...frames.values()];
      frames.clear();
      run.forEach((f) => f());
      return run.length;
    },
    pending: () => frames.size,
    watchers: () => watchers,
    unwatched: () => unwatched,
    /** Mounts and waits for the application to start. */
    async mount() {
      const dispose = driver.mount(asHost(host), surface);
      await vi.advanceTimersByTimeAsync(0);
      return dispose;
    },
  };
}
