// The driver of the canvas, with no browser: a fake Pixi application, a fake host element and a clock the test turns by hand.
// It is the one place that holds what the Svelte and the React canvas share (the frame loop, the culling, the controlled camera,
// the mount and its cleanup), so both are checked here and neither copy repeats it.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Container } from 'pixi.js';
import { HOME, sameCamera, type Camera } from '@meldframe/web-canvas/camera';
import type { Scene } from '@meldframe/web-canvas/scene';
import { pixiColors } from '@meldframe/web-tokens/tokens';
import { WorldView } from './stage';
import { ASSET_RETRY_EVERY_MS, type CanvasEvents, type CanvasProps, type HostWatchers, type Platform } from './api';
import { CanvasDriver } from './driver';

const rect = (id: string, x: number, y = 0) => ({ id, parent: null, kind: 'rect' as const, x, y, w: 100, h: 100, fill: 0x808080 });
const scene = (...nodes: ReturnType<typeof rect>[]): Scene => ({ space: 'parent', nodes });
const props = (over: Partial<CanvasProps> = {}): CanvasProps => ({
  scene: scene(), selection: [], camera: { ...HOME }, touches: [], indicator: null, tool: 'move', theme: 'dark', ...over,
});

function fakeApp(failWith?: Error) {
  const stage = new Container();
  const renderer = { name: 'fake', resolution: 1, background: { color: -1 }, render: vi.fn(), resize: vi.fn() };
  return {
    stage, renderer, ticker: { stop: vi.fn() }, destroy: vi.fn(),
    init: vi.fn(async () => {
      if (failWith) throw failWith;
    }),
  };
}
type FakeApp = ReturnType<typeof fakeApp>;

/** An element that keeps its listeners (the `signal` of `addEventListener` removes them, as in a browser). */
function fakeHost() {
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
type FakeHost = ReturnType<typeof fakeHost>;
const asHost = (h: FakeHost) => h as unknown as HTMLElement;
const surface = {} as HTMLCanvasElement;

function setup(app: FakeApp = fakeApp()) {
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

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('starting the canvas', () => {
  it('draws nothing and fails nowhere before it is mounted; the first scene is applied when the application is ready', async () => {
    const t = setup();
    t.driver.setProps(props({ scene: scene(rect('a', 0)) }));
    expect(t.driver.stats().nodes).toBe(0);
    expect(t.app.init).not.toHaveBeenCalled();
    await t.mount();
    expect(t.driver.stats().nodes).toBe(1);
    expect(t.events.onReady).toHaveBeenCalledWith({ renderer: 'fake' });
    expect(t.events.onStatus).toHaveBeenLastCalledWith({ renderer: 'fake', failed: false });
  });

  it('starts the application on the surface with the size of the host, the density and the theme background, and does not tick by itself', async () => {
    const t = setup();
    t.driver.setProps(props({ theme: 'light' }));
    await t.mount();
    expect(t.app.init).toHaveBeenCalledWith(
      expect.objectContaining({ canvas: surface, width: 800, height: 600, resolution: 1, autoDensity: true, antialias: true, backgroundColor: pixiColors('light').canvas, autoStart: false, sharedTicker: false, preference: ['webgl', 'canvas'] }),
    );
    expect(t.app.ticker.stop).toHaveBeenCalled();
  });

  it('reports a start that fails once, as an error and as a status, and calls nothing else', async () => {
    const boom = new Error('no webgl');
    const t = setup(fakeApp(boom));
    await t.mount();
    expect(t.events.onError).toHaveBeenCalledWith(boom);
    expect(t.events.onStatus).toHaveBeenLastCalledWith({ renderer: '', failed: true });
    expect(t.events.onReady).not.toHaveBeenCalled();
    expect(t.app.destroy).toHaveBeenCalledWith({ removeView: false }, { children: true });
    expect(t.driver.stats().renderer).toBe('');
  });

  it('a mount that is undone before the application has started never becomes ready and releases the application', async () => {
    const t = setup();
    const dispose = t.driver.mount(asHost(t.host), surface);
    dispose();
    await vi.advanceTimersByTimeAsync(0);
    expect(t.events.onReady).not.toHaveBeenCalled();
    expect(t.app.destroy).toHaveBeenCalledTimes(1);
    expect(t.host.count()).toBe(0);
    expect(vi.getTimerCount()).toBe(0);
  });

  it('a start that fails after the mount was undone says nothing', async () => {
    const t = setup(fakeApp(new Error('late')));
    t.driver.mount(asHost(t.host), surface)();
    await vi.advanceTimersByTimeAsync(0);
    expect(t.events.onError).not.toHaveBeenCalled();
  });
});

describe('mount and cleanup are a pair that can be repeated (React runs an effect twice in development)', () => {
  it('leaves no listener, observer, timer or application behind, and a second mount on the same element works', async () => {
    const first = setup();
    const dispose = await first.mount();
    first.driver.setProps(props({ scene: scene(rect('a', 0)) }));
    expect(first.host.count()).toBeGreaterThan(0);
    dispose();
    expect(first.host.count()).toBe(0);
    expect(first.unwatched()).toBe(1);
    expect(first.app.destroy).toHaveBeenCalledTimes(1);
    expect(vi.getTimerCount()).toBe(0);
    expect(first.driver.stats().nodes).toBe(0);

    // The same driver, the same host, a new application: ready again, with the props it already holds.
    const second = fakeApp();
    (first.platform as { createApp: () => unknown }).createApp = () => second;
    await vi.advanceTimersByTimeAsync(0);
    const again = first.driver.mount(asHost(first.host), surface);
    await vi.advanceTimersByTimeAsync(0);
    expect(second.init).toHaveBeenCalledTimes(1);
    expect(first.driver.stats().nodes).toBe(1);
    expect(first.events.onReady).toHaveBeenCalledTimes(2);
    again();
    expect(first.host.count()).toBe(0);
  });

  it('cleanup twice is harmless', async () => {
    const t = setup();
    const dispose = await t.mount();
    dispose();
    expect(() => dispose()).not.toThrow();
    expect(t.app.destroy).toHaveBeenCalledTimes(1);
  });

  it('a status of a first mount is cleared when the next one starts', async () => {
    const t = setup();
    const dispose = await t.mount();
    dispose();
    const next = t.driver.mount(asHost(t.host), surface);
    expect(t.events.onStatus).toHaveBeenLastCalledWith({ renderer: '', failed: false });
    next();
  });
});

describe('frames', () => {
  it('asks for one frame however often it is asked, draws it once, and draws again only when asked again', async () => {
    const t = setup();
    await t.mount();
    t.frame(); // the frame the start asked for
    t.driver.setProps(props({ selection: ['a'] }));
    t.driver.setProps(props({ selection: ['a', 'b'] }));
    expect(t.pending()).toBe(1);
    const before = t.app.renderer.render.mock.calls.length;
    t.frame();
    expect(t.app.renderer.render).toHaveBeenCalledTimes(before + 1);
    expect(t.pending()).toBe(0);
    expect(t.driver.stats().timing.frames).toBeGreaterThanOrEqual(2);
  });

  it('draws again when only what the overlay draws from changes (selection, touches, indicator, tool, theme, reduced motion), and not when the host gives the same values again', async () => {
    const t = setup();
    const base = props();
    t.driver.setProps(base);
    await t.mount();
    t.frame();
    t.driver.setProps({ ...base }); // the same values in another object: nothing changed
    expect(t.pending()).toBe(0);
    for (const over of [{ selection: ['a'] }, { touches: [] }, { indicator: { x: 0, y: 0, w: 1, h: 0 } }, { tool: 'rect' as const }, { theme: 'light' as const }, { reducedMotion: true }]) {
      t.driver.setProps({ ...base, ...over });
      expect(t.pending(), JSON.stringify(over)).toBe(1);
      t.frame();
      t.driver.setProps(base);
      t.frame();
    }
  });

  it('draws nothing by itself while nothing changes', async () => {
    const t = setup();
    await t.mount();
    t.frame();
    expect(t.frame()).toBe(0);
  });

  it('a theme change sets the background of the application', async () => {
    const t = setup();
    await t.mount();
    t.driver.setProps(props({ theme: 'light' }));
    expect(t.app.renderer.background.color).toBe(pixiColors('light').canvas);
    t.driver.setProps(props({ theme: 'dark' }));
    expect(t.app.renderer.background.color).toBe(pixiColors('dark').canvas);
  });

  it('follows the reduced-motion setting of the system unless the host says otherwise, and draws again when it changes', async () => {
    const t = setup();
    await t.mount();
    t.frame();
    t.watchers()!.reducedMotion(true);
    expect(t.pending()).toBe(1);
  });

  it('a change of density sets the resolution and resizes; a change of fonts draws the text again', async () => {
    const t = setup();
    await t.mount();
    t.frame();
    t.watchers()!.density(2);
    expect(t.app.renderer.resolution).toBe(2);
    expect(t.app.renderer.resize).toHaveBeenCalledWith(800, 600);
    t.frame();
    const refresh = vi.spyOn(WorldView.prototype, 'refreshText');
    t.watchers()!.fonts();
    expect(refresh).toHaveBeenCalledTimes(1);
    expect(t.pending()).toBe(1);
    refresh.mockRestore();
  });

  it('resizes the application to the host, at least one pixel', async () => {
    const t = setup();
    await t.mount();
    t.platform.measure = () => ({ w: 0, h: 0 });
    t.watchers()!.resize();
    expect(t.app.renderer.resize).toHaveBeenLastCalledWith(1, 1);
  });
});

describe('culling', () => {
  const far = scene(rect('near', 0), rect('far', 5000));

  it('draws what is in the view, and a node that comes into the view when the camera moves', async () => {
    const t = setup();
    t.driver.setProps(props({ scene: far }));
    await t.mount();
    t.frame();
    expect(t.driver.stats().nodes).toBe(2);
    expect(t.driver.stats().drawn).toBe(1);
    t.driver.setProps(props({ scene: far, camera: { x: -5000, y: 0, zoom: 1 } }));
    t.frame();
    expect(t.driver.stats().drawn).toBe(1); // the far one now, the near one is out of the view
    t.driver.setProps(props({ scene: far, camera: { x: 145, y: 0, zoom: 0.1 } }));
    t.frame();
    expect(t.driver.stats().drawn).toBe(2);
  });

  it('a node that is added, moved or turned visible is culled again although the camera stays', async () => {
    const t = setup();
    t.driver.setProps(props({ scene: scene(rect('a', 0)) }));
    await t.mount();
    t.frame();
    t.driver.setProps(props({ scene: scene(rect('a', 0), rect('b', 200)) }));
    t.frame();
    expect(t.driver.stats().drawn).toBe(2);
  });
});

describe('the camera, a controlled prop with a callback', () => {
  const reported = (t: ReturnType<typeof setup>): Camera[] => t.events.onCameraChange.mock.calls.map((c) => c[0] as Camera);

  it('applies a camera the host gives, without reporting it back', async () => {
    const t = setup();
    await t.mount();
    t.driver.setProps(props({ camera: { x: 10, y: 20, zoom: 2 } }));
    expect(t.driver.centre()).toEqual({ x: (400 - 10) / 2, y: (300 - 20) / 2 });
    expect(t.events.onCameraChange).not.toHaveBeenCalled();
  });

  it('reports a move of its own at once, and holds it when the host hands the same camera back', async () => {
    const t = setup();
    t.driver.setProps(props({ scene: scene(rect('a', 0)) }));
    await t.mount();
    const from = t.driver.centre();
    t.driver.fitAll();
    expect(t.events.onCameraChange).toHaveBeenCalledTimes(1);
    const [fit, final] = t.events.onCameraChange.mock.calls[0]!;
    expect(final).toBe(true);
    const centre = t.driver.centre();
    expect(centre).not.toEqual(from);
    t.driver.setProps(props({ scene: scene(rect('a', 0)), camera: { ...fit } })); // the echo
    expect(t.driver.centre()).toEqual(centre);
    expect(t.events.onCameraChange).toHaveBeenCalledTimes(1);
  });

  it('does not take back a camera it moved while the host still holds the old one', async () => {
    const t = setup();
    t.driver.setProps(props({ scene: scene(rect('a', 0)), camera: { x: 5, y: 5, zoom: 1 } }));
    await t.mount();
    t.driver.fitAll();
    const moved = t.driver.centre();
    // The host re-renders for another reason and still holds the camera it gave: it did not change its mind.
    t.driver.setProps(props({ scene: scene(rect('a', 0)), camera: { x: 5, y: 5, zoom: 1 }, selection: ['a'] }));
    expect(t.driver.centre()).toEqual(moved);
    // A camera that differs from the one it gave before is the host's choice.
    t.driver.setProps(props({ scene: scene(rect('a', 0)), camera: { x: 0, y: 0, zoom: 1 }, selection: ['a'] }));
    expect(t.driver.centre()).toEqual({ x: 400, y: 300 });
  });

  it('a wheel pans and zooms: each step is reported as not final and one final report comes when it has settled', async () => {
    const t = setup();
    await t.mount();
    t.host.emit('wheel', { clientX: 100, clientY: 100, deltaX: 30, deltaY: 40, deltaMode: 0, ctrlKey: false, metaKey: false });
    t.host.emit('wheel', { clientX: 100, clientY: 100, deltaX: 30, deltaY: 40, deltaMode: 0, ctrlKey: false, metaKey: false });
    expect(t.events.onCameraChange.mock.calls.map((c) => c[1])).toEqual([false, false]);
    expect(sameCamera(reported(t)[0]!, reported(t)[1]!)).toBe(false);
    await vi.advanceTimersByTimeAsync(119);
    expect(t.events.onCameraChange).toHaveBeenCalledTimes(2);
    await vi.advanceTimersByTimeAsync(2);
    expect(t.events.onCameraChange.mock.calls.map((c) => c[1])).toEqual([false, false, true]);
    expect(sameCamera(reported(t)[2]!, reported(t)[1]!)).toBe(true);
  });

  it('the host callback is read when it is called, so a new one replaces the old', async () => {
    const t = setup();
    await t.mount();
    const next = vi.fn();
    t.driver.events = { ...t.events, onCameraChange: next };
    t.host.emit('wheel', { clientX: 1, clientY: 1, deltaX: 5, deltaY: 0, deltaMode: 0, ctrlKey: false, metaKey: false });
    expect(next).toHaveBeenCalledTimes(1);
    expect(t.events.onCameraChange).not.toHaveBeenCalled();
  });

  it('shows the selection, or all the content when nothing is selected, and the centre of the view', async () => {
    const t = setup();
    t.driver.setProps(props({ scene: scene(rect('a', 0), rect('b', 3000)), selection: ['b'] }));
    await t.mount();
    t.driver.fitSelection();
    const onB = t.driver.centre();
    expect(onB.x).toBeCloseTo(3050, 0);
    t.driver.setProps(props({ scene: scene(rect('a', 0), rect('b', 3000)), selection: [] }));
    t.driver.fitSelection();
    expect(t.driver.centre().x).toBeCloseTo(1550, 0);
  });
});

describe('input becomes intents of the host', () => {
  it('a click on a node selects it, and a double click with the move tool activates it; the tool is read when the event comes', async () => {
    const t = setup();
    const onSelect = vi.fn();
    const onActivate = vi.fn();
    t.driver.events = { ...t.events, onSelect, onActivate };
    t.driver.setProps(props({ scene: scene(rect('a', 0)) }));
    await t.mount();
    const down = { pointerId: 1, clientX: 50, clientY: 50, button: 0, pointerType: 'mouse', shiftKey: false, altKey: false, ctrlKey: false, metaKey: false };
    t.host.emit('pointerdown', down);
    t.host.emit('pointerup', down);
    expect(onSelect).toHaveBeenCalledWith(['a'], false);
    t.host.emit('dblclick', { clientX: 50, clientY: 50 });
    expect(onActivate).toHaveBeenCalledWith('a');
    t.driver.setProps(props({ scene: scene(rect('a', 0)), tool: 'rect' }));
    t.host.emit('dblclick', { clientX: 50, clientY: 50 });
    expect(onActivate).toHaveBeenCalledTimes(1);
  });

  it('after the cleanup the element no longer answers', async () => {
    const t = setup();
    const onSelect = vi.fn();
    t.driver.events = { ...t.events, onSelect };
    t.driver.setProps(props({ scene: scene(rect('a', 0)) }));
    const dispose = await t.mount();
    dispose();
    const down = { pointerId: 1, clientX: 50, clientY: 50, button: 0, pointerType: 'mouse', shiftKey: false, altKey: false, ctrlKey: false, metaKey: false };
    t.host.emit('pointerdown', down);
    t.host.emit('pointerup', down);
    expect(onSelect).not.toHaveBeenCalled();
  });

  it('takes the keyboard back to the host element', async () => {
    const t = setup();
    await t.mount();
    t.driver.focus();
    expect(t.host.focus).toHaveBeenCalledWith({ preventScroll: true });
  });
});

describe('capture', () => {
  it('says the canvas is not ready before it has started and after the cleanup', async () => {
    const t = setup();
    await expect(t.driver.capture({ x: 0, y: 0, w: 10, h: 10 }, 1)).rejects.toThrow('The canvas is not ready.');
    const dispose = await t.mount();
    dispose();
    await expect(t.driver.capture({ x: 0, y: 0, w: 10, h: 10 }, 1)).rejects.toThrow('The canvas is not ready.');
  });

  it('draws the region at the scale, hands over the PNG, and culls again for the view afterwards', async () => {
    const t = setup();
    t.driver.setProps(props({ scene: scene(rect('a', 0)) }));
    await t.mount();
    t.frame();
    const image = { toBlob: (cb: (b: Blob | null) => void) => cb(new Blob(['x'], { type: 'image/png' })) } as unknown as HTMLCanvasElement;
    const region = vi.spyOn(WorldView.prototype, 'renderRegion').mockReturnValue(image);
    const blob = await t.driver.capture({ x: 0, y: 0, w: 10.2, h: 5 }, 2);
    expect(blob.type).toBe('image/png');
    expect(region).toHaveBeenCalledWith(t.app.renderer, { x: 0, y: 0, w: 10.2, h: 5 }, 2, 21, 10);
    expect(t.pending()).toBe(1);
    region.mockRestore();
  });

  it('gives the box of a node, or of all the content, with the band of a border', async () => {
    const t = setup();
    t.driver.setProps(props({ scene: scene(rect('a', 0), rect('b', 300)) }));
    await t.mount();
    expect(t.driver.captureBox('a')).toEqual({ x: 0, y: 0, w: 100, h: 100 });
    expect(t.driver.captureBox()).toEqual({ x: 0, y: 0, w: 400, h: 100 });
    expect(t.driver.captureBox('missing')).toBeNull();
  });
});

describe('the asset retry timer', () => {
  it('runs while the canvas is mounted and stops with it', async () => {
    const t = setup();
    const dispose = await t.mount();
    expect(vi.getTimerCount()).toBeGreaterThan(0);
    await vi.advanceTimersByTimeAsync(ASSET_RETRY_EVERY_MS * 2);
    dispose();
    expect(vi.getTimerCount()).toBe(0);
  });
});
