// The driver of the canvas: everything that is not markup. It starts the Pixi application on a surface, keeps it in step with
// the props it is given (the scene, the selection, the camera, the theme), runs the frame loop, culls and sharpens when the view
// settles, turns the host element's input into intents, and answers a capture. It is plain TypeScript with no framework in it:
// `Canvas.svelte` and the React copy `Canvas.tsx` (E7.3, S11) are thin wrappers that hand it their props, their callbacks and
// an element, so the rules are here once and are tested without a browser (driver.test.ts, with a fake application and a fake
// host). It holds no document state and applies no edit: a move or resize is reported in world units and the host decides what it
// means (U1, U2). Colours come from the tokens module; the colours of the nodes are document data inside the scene.
//
// The camera is a controlled prop with a callback. The canvas moves its own camera at once (the next pointer event must see it)
// and reports it through `onCameraChange`; the host hands the same camera back and nothing happens, or hands another and the
// canvas moves to it. The host's camera is applied only when it differs from the one it gave before, so a host that re-renders for
// another reason cannot take back a move the canvas made meanwhile.
import type { Application } from 'pixi.js';
import { HOME, fitRect, rectToWorld, sameCamera, screenToWorld, visibleWorldRect, type Camera } from '@meldframe/web-canvas/camera';
import { containsRect, unionRects, type Rect, type Size } from '@meldframe/web-canvas/geometry';
import { Interaction, type Env, type Intents } from '@meldframe/web-canvas/interaction';
import { attachInput } from '@meldframe/web-input/hostinput';
import { selectionBox } from '@meldframe/web-canvas/overlay';
import { EMPTY_SCENE, captureBounds, captureRectOf, contentBounds, rectsOf, resolveScene, type Scene } from '@meldframe/web-canvas/scene';
import { AssetCache } from '@meldframe/web-assets/assets';
import { duration, pixiColors } from '@meldframe/web-tokens/tokens';
import { ASSET_RETRY_EVERY_MS, ASSET_TIMEOUT_MS, type CanvasEvents, type CanvasProps, type CanvasStatus, type FrameTiming, type Platform } from './api';
import { OverlayView } from './overlayview';
import { WorldView } from './stage';

export type { CanvasEvents, CanvasProps, CanvasStatus, FrameTiming, HostWatchers, Platform } from './api';

/** What a host may call on a mounted canvas. */
export type CanvasHandle = Pick<CanvasDriver, 'fitAll' | 'fitSelection' | 'centre' | 'focus' | 'captureBox' | 'capture' | 'stats'>;

// The scene the props hold before the host gives one. It is a `Scene` (the host's form), not the `EMPTY_SCENE` of the module `scene`, which is a
// `ResolvedScene`; its identity matters, since a scene is told changed by identity and the first real one must differ from it.
const NO_SCENE: Scene = { space: 'parent', nodes: [] };

export class CanvasDriver {
  events: CanvasEvents = {};

  private props: CanvasProps = { scene: NO_SCENE, selection: [], camera: { ...HOME }, touches: [], indicator: null, tool: 'move', theme: 'dark' };
  /** The camera the host gave last, to tell a change of the host's mind from a re-render. */
  private given: Camera | null = null;
  private camera: Camera = { ...HOME };
  private palette = pixiColors('dark');
  private systemReduced = false;
  private status: CanvasStatus = { renderer: '', failed: false };

  // Not reactive on purpose: the Pixi objects are driven from the frame loop.
  private host: HTMLElement | null = null;
  private app: Application | null = null;
  private world: WorldView | null = null;
  private overlay: OverlayView | null = null;
  private assets: AssetCache | null = null;
  private retryTimer: ReturnType<typeof setInterval> | undefined;
  private ready = false;
  private resolved = EMPTY_SCENE;
  private viewport: Size = { w: 0, h: 0 };
  private dpr = 1;
  private hover: string | null = null;
  private draft: Rect | null = null;
  private raf = 0;
  private needCull = false;
  private culledWorld: Rect | null = null;
  private settleTimer: ReturnType<typeof setTimeout> | undefined;
  private tightenTimer: ReturnType<typeof setTimeout> | undefined;
  private timing: FrameTiming = { cull: 0, overlay: 0, render: 0, frames: 0 };

  private readonly env: Env = {
    camera: () => this.camera,
    selection: () => this.props.selection,
    tool: () => this.props.tool,
    viewport: () => this.viewport,
    hit: (x, y) => this.hitAt(x, y),
    selectionBox: () => selectionBox(this.camera, rectsOf(this.resolved, this.props.selection)),
  };

  private readonly intents: Intents = {
    select: (ids, additive) => this.events.onSelect?.(ids, additive),
    move: (i) => this.events.onMove?.(i),
    resize: (i) => this.events.onResize?.(i),
    create: (i) => {
      this.draft = i.final ? null : { x: i.x, y: i.y, w: i.w, h: i.h };
      this.events.onCreate?.(i);
      this.schedule();
    },
    camera: (c, final) => this.setCamera(c, final),
    hover: (id) => {
      this.hover = id;
      this.events.onHover?.(id);
      this.schedule();
    },
    fit: (kind) => (kind === 'selection' ? this.fitSelection() : this.fitAll()),
  };

  private readonly interaction = new Interaction(this.env, this.intents);

  constructor(private readonly platform: Platform) {}

  /** What is under a world point, as the host says (the app layer's rule; the canvas has none). */
  private hitAt(x: number, y: number): string | null {
    return this.events.hitAt?.(x, y) ?? null;
  }

  private get reduced(): boolean {
    return this.props.reducedMotion ?? this.systemReduced;
  }

  // ---- props -> pixi ------------------------------------------------------------------------------

  /** Gives the canvas what it draws from; it works out what changed. Safe before `mount` (the first values wait) and any number of times. */
  setProps(p: CanvasProps): void {
    const prev = this.props;
    this.props = p;
    if (p.theme !== prev.theme) this.palette = pixiColors(p.theme);
    if (p.scene !== prev.scene) this.applyScene();
    if (!this.given || !sameCamera(p.camera, this.given)) {
      this.given = { ...p.camera };
      if (!sameCamera(p.camera, this.camera)) {
        this.camera = { ...p.camera };
        this.cameraChanged();
      }
    }
    if (p.theme !== prev.theme) this.applyBackground();
    // Anything the overlay draws from: a change only needs another frame.
    if (p.selection !== prev.selection || p.touches !== prev.touches || p.indicator !== prev.indicator || p.tool !== prev.tool || p.theme !== prev.theme || p.reducedMotion !== prev.reducedMotion) {
      this.schedule();
    }
  }

  private applyScene(): void {
    if (!this.ready || !this.world) return;
    const r = resolveScene(this.props.scene, this.resolved);
    this.resolved = r.scene;
    this.world.apply(r.scene, r.patch);
    // A node that appears, or moves, resizes or turns visible, may enter the view while the camera stays still.
    if (r.patch.added.length || r.patch.changed.length) this.needCull = true;
    this.schedule();
  }

  /** The camera moved, by the host or by the canvas: show it, and tighten the culling once it stops (zooming in leaves the old, wider cull valid). */
  private cameraChanged(): void {
    if (!this.ready || !this.world) return;
    const c = this.camera;
    this.world.setCamera(c);
    if (!this.culledWorld || !containsRect(this.culledWorld, visibleWorldRect(c, this.viewport))) this.needCull = true;
    clearTimeout(this.tightenTimer);
    this.tightenTimer = setTimeout(() => this.tighten(), duration.fast);
    this.schedule();
  }

  private applyBackground(): void {
    if (!this.ready || !this.app) return;
    this.app.renderer.background.color = this.palette.canvas;
    this.schedule();
  }

  // ---- frames -------------------------------------------------------------------------------------

  private readonly schedule = (): void => {
    if (!this.raf && this.app) this.raf = this.platform.requestFrame(this.frame);
  };

  private readonly frame = (): void => {
    this.raf = 0;
    const { app, world, overlay } = this;
    if (!app || !world || !overlay) return;
    const now = performance.now();
    this.timing.cull = 0;
    if (this.needCull) {
      this.needCull = false;
      this.cullNow();
    }
    const t0 = performance.now();
    const p = this.props;
    overlay.draw({
      camera: this.camera, viewport: this.viewport, scene: this.resolved, selection: p.selection, hover: this.hover, touches: p.touches,
      draft: this.draft, indicator: p.indicator, palette: this.palette, reducedMotion: this.reduced, now,
    });
    const t1 = performance.now();
    app.renderer.render(app.stage);
    this.timing.overlay = t1 - t0;
    this.timing.render = performance.now() - t1;
    this.timing.frames++;
    if (OverlayView.animating(p.touches, now, this.reduced)) this.schedule();
  };

  /** Hides what is outside the view, with half a viewport of margin so a slow pan does not show gaps. */
  private cullNow(): void {
    if (!this.world) return;
    const mx = this.viewport.w / 2;
    const my = this.viewport.h / 2;
    const view = { x: -mx, y: -my, width: this.viewport.w + 2 * mx, height: this.viewport.h + 2 * my };
    const t = performance.now();
    this.world.cull(view);
    this.timing.cull = performance.now() - t;
    this.culledWorld = rectToWorld(this.camera, { x: view.x, y: view.y, w: view.width, h: view.height });
  }

  /** Pan or zoom has settled: tighten the culling and sharpen text for the new zoom. */
  private tighten(): void {
    if (!this.world) return;
    this.cullNow();
    this.world.sharpenText(this.camera.zoom, this.dpr);
    this.schedule();
  }

  /** `tighten`, and the report of the final camera that `settleSoon` was waiting to make is no longer needed. */
  private settle(): void {
    clearTimeout(this.settleTimer);
    this.tighten();
  }

  private readonly settleSoon = (): void => {
    clearTimeout(this.settleTimer);
    this.settleTimer = setTimeout(() => {
      this.events.onCameraChange?.(this.camera, true);
      this.settle();
    }, duration.fast);
  };

  /** The canvas moves its camera: at once for the next event, and told to the host. */
  private setCamera(c: Camera, final: boolean): void {
    this.camera = c;
    this.events.onCameraChange?.(c, final);
    // Cull against the new transform, not the one the next frame has not applied yet.
    if (final) {
      this.world?.setCamera(c);
      this.settle();
    }
    this.cameraChanged();
  }

  // ---- what a host may call -----------------------------------------------------------------------

  /** Shows all the content. */
  fitAll(): void {
    this.setCamera(fitRect(contentBounds(this.resolved), this.viewport), true);
  }

  /** Shows the selection, or all the content when nothing is selected. */
  fitSelection(): void {
    const box = unionRects(rectsOf(this.resolved, this.props.selection));
    if (!box) this.fitAll();
    else this.setCamera(fitRect(box, this.viewport), true);
  }

  /** The world point at the centre of what the canvas shows. */
  centre(): { x: number; y: number } {
    return screenToWorld(this.camera, this.viewport.w / 2, this.viewport.h / 2);
  }

  /** Gives the keyboard back to the canvas (after an editor laid over it closes). */
  focus(): void {
    this.host?.focus({ preventScroll: true });
  }

  /** The box in world units of a node, or of all the content when no id is given, with the band of a border that lies outside; null when the node is not here. */
  captureBox(id?: string): Rect | null {
    return id === undefined ? captureBounds(this.resolved) : captureRectOf(this.resolved, id);
  }

  /** The world in `box` at `scale` as a PNG, without the overlay; the view is drawn again afterwards. */
  async capture(box: Rect, scale: number): Promise<Blob> {
    if (!this.app || !this.world) throw new Error('The canvas is not ready.');
    // Every picture the view asked for is in the image (a missing one is drawn as its fill, as on the screen), unless one
    // takes too long: the capture then shows that node by its fill.
    await this.assets?.idle(ASSET_TIMEOUT_MS);
    if (!this.app || !this.world) throw new Error('The canvas is not ready.');
    const width = Math.ceil(box.w * scale);
    const height = Math.ceil(box.h * scale);
    let image: HTMLCanvasElement;
    try {
      image = this.world.renderRegion(this.app.renderer, box, scale, width, height);
    } finally {
      this.needCull = true; // the drawing showed everything: cull again for the view
      this.schedule();
    }
    return new Promise((resolve, reject) => image.toBlob((b) => (b ? resolve(b) : reject(new Error('The image could not be encoded.'))), 'image/png'));
  }

  /** What the canvas is drawing now; for measurements and tests. */
  stats(): { nodes: number; drawn: number; renderer: string; dpr: number; timing: FrameTiming } {
    return { timing: { ...this.timing }, nodes: this.world?.nodeCount ?? 0, drawn: this.world?.countRenderable() ?? 0, renderer: this.app?.renderer.name ?? '', dpr: this.dpr };
  }

  // ---- mount --------------------------------------------------------------------------------------

  private setStatus(next: CanvasStatus): void {
    if (next.renderer === this.status.renderer && next.failed === this.status.failed) return;
    this.status = next;
    this.events.onStatus?.(next);
  }

  private resize(): void {
    if (!this.host) return;
    this.viewport = this.measure(this.host);
    this.app?.renderer.resize(this.viewport.w, this.viewport.h);
    this.needCull = true;
    this.schedule();
  }

  private measure(host: HTMLElement): Size {
    const r = this.platform.measure(host);
    return { w: Math.max(1, Math.round(r.w)), h: Math.max(1, Math.round(r.h)) };
  }

  /**
   * Starts the canvas on `host` (the element that takes the input and has the size) and `surface` (the element Pixi draws on),
   * and returns the function that undoes it. Mount and cleanup are a pair that can be repeated on the same driver and elements
   * (React runs an effect twice in development): the cleanup releases everything the mount made, and an application that was still
   * starting when it ran is destroyed when it has started, never used.
   */
  mount(host: HTMLElement, surface: HTMLCanvasElement): () => void {
    let disposed = false;
    this.host = host;
    this.setStatus({ renderer: '', failed: false });
    const detachInput = attachInput(host, {
      interaction: this.interaction,
      tool: () => this.props.tool,
      camera: () => this.camera,
      hit: (x, y) => this.hitAt(x, y),
      schedule: this.schedule,
      dropDraft: () => (this.draft = null),
      settleSoon: this.settleSoon,
      activate: (id) => this.events.onActivate?.(id),
    });
    this.dpr = this.platform.dpr();
    // A web font that arrives after the first draw: draw the text again.
    const unwatch = this.platform.watch(host, {
      resize: () => this.resize(),
      reducedMotion: (on) => {
        this.systemReduced = on;
        if (this.ready) this.schedule();
      },
      density: (dpr) => {
        this.dpr = dpr;
        if (this.app) this.app.renderer.resolution = dpr;
        this.resize();
      },
      fonts: () => {
        this.world?.refreshText();
        this.schedule();
      },
    });
    void this.start(host, surface, () => disposed);

    return () => {
      if (disposed) return;
      disposed = true;
      unwatch();
      detachInput();
      clearTimeout(this.settleTimer);
      clearTimeout(this.tightenTimer);
      if (this.raf) this.platform.cancelFrame(this.raf);
      this.raf = 0;
      this.ready = false;
      this.overlay?.destroy();
      this.world?.destroy();
      clearInterval(this.retryTimer);
      this.assets?.destroy();
      this.assets = null;
      this.app?.destroy({ removeView: false }, { children: true });
      this.app = null;
      this.world = null;
      this.overlay = null;
      this.resolved = EMPTY_SCENE;
      this.culledWorld = null;
      this.needCull = false;
      this.hover = null;
      this.draft = null;
      this.host = null;
    };
  }

  private async start(host: HTMLElement, surface: HTMLCanvasElement, isDisposed: () => boolean): Promise<void> {
    const a = this.platform.createApp();
    try {
      this.viewport = this.measure(host);
      await a.init({
        canvas: surface,
        width: this.viewport.w,
        height: this.viewport.h,
        resolution: this.dpr,
        autoDensity: true,
        antialias: true,
        backgroundColor: this.palette.canvas,
        // WebGL, and the 2D canvas renderer of Pixi when WebGL is not available.
        preference: ['webgl', 'canvas'],
        autoStart: false,
        sharedTicker: false,
      });
    } catch (err) {
      if (!isDisposed()) {
        this.setStatus({ renderer: '', failed: true });
        this.events.onError?.(err);
      }
      try {
        a.destroy({ removeView: false }, { children: true });
      } catch {
        // init failed half way: nothing more to release
      }
      return;
    }
    if (isDisposed()) {
      a.destroy({ removeView: false }, { children: true });
      return;
    }
    this.app = a;
    a.ticker.stop();
    // The images of image fills: fetched through the same origin (the dev proxy adds the token), decoded off the main thread.
    const assets = new AssetCache(this.platform.assets);
    this.assets = assets;
    // An image that failed a while ago (the server was restarting, the image was not uploaded yet) is asked for again.
    this.retryTimer = setInterval(() => this.assets?.retryProblems(), ASSET_RETRY_EVERY_MS);
    assets.onProblem = (id, code) => this.events.onAssetProblem?.(id, code);
    assets.onChange((id) => {
      this.world?.repaintAsset(id);
      this.needCull = true;
      this.schedule();
    });
    const world = new WorldView(assets);
    const overlay = new OverlayView();
    this.world = world;
    this.overlay = overlay;
    a.stage.addChild(world.root, overlay.root);
    a.stage.eventMode = 'none';
    world.setCamera(this.camera);
    this.ready = true;
    this.resize(); // the observer may have fired during the async init, before the app existed
    this.setStatus({ renderer: a.renderer.name, failed: false });
    // What the props asked for while the application started.
    this.applyScene();
    this.cameraChanged();
    this.applyBackground();
    this.schedule();
    this.events.onReady?.({ renderer: a.renderer.name });
  }
}
