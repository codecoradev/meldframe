<!--
  The canvas of the editor: draws the scene it is given with PixiJS and reports what the user does as
  intents. It holds no document state and applies no edit: a move or resize is reported in world units
  and the host decides what it means (U1, U2). Colours come from the tokens module; the colours of the
  nodes themselves are document data inside the scene.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { Application } from 'pixi.js';
  import { HOME, fitRect, rectToWorld, screenToWorld, visibleWorldRect, type Camera } from '@meldframe/web-canvas/camera';
  import { containsRect, unionRects, type Rect, type Size } from '@meldframe/web-canvas/geometry';
  import {
    Interaction, type CreateIntent, type Env, type Intents, type MoveIntent, type ResizeIntent, type ToolMode,
  } from '@meldframe/web-canvas/interaction';
  import { attachInput } from '@meldframe/web-input/hostinput';
  import { OverlayView, type Touch } from './overlayview';
  import { selectionBox } from '@meldframe/web-canvas/overlay';
  import { EMPTY_SCENE, contentBounds, hitTest, rectsOf, resolveScene, type Scene } from '@meldframe/web-canvas/scene';
  import { WorldView } from './stage';
  import { AssetCache } from '@meldframe/web-assets/assets';
  import { duration, pixiColors, type Theme } from '@meldframe/web-tokens/tokens';

  interface Props {
    scene: Scene;
    /** Ids drawn as selected. */
    selection?: readonly string[];
    /** The camera; the canvas changes it for pan, zoom and fit, and says so through `onCameraChange`. */
    camera?: Camera;
    touches?: readonly Touch[];
    /** Where a dragged node would land in a flow: a line in world units, or none. */
    indicator?: Rect | null;
    tool?: ToolMode;
    theme?: Theme;
    /** Overrides the system's reduced-motion setting when set. */
    reducedMotion?: boolean;
    /** Accessible name of the canvas. */
    label?: string;
    /** `additive` is true with Shift held: the host decides whether that adds or toggles. */
    onSelect?: (ids: string[], additive: boolean) => void;
    onMove?: (i: MoveIntent) => void;
    onResize?: (i: ResizeIntent) => void;
    onCreate?: (i: CreateIntent) => void;
    /** `final` is false while the camera is still moving. */
    onCameraChange?: (camera: Camera, final: boolean) => void;
    onHover?: (id: string | null) => void;
    /** A double click on a node (with the select tool): the id of the node under the pointer. */
    onActivate?: (id: string) => void;
    /** The renderer that started, or the error when none could. */
    onReady?: (info: { renderer: string }) => void;
    onError?: (error: unknown) => void;
    /** An image of an image fill is missing or could not be loaded: its asset id and the stable error code. */
    onAssetProblem?: (id: string, code: string) => void;
  }

  let {
    scene,
    selection = [],
    camera = $bindable({ ...HOME }),
    touches = [],
    indicator = null,
    tool = 'move',
    theme = 'dark',
    reducedMotion,
    label = 'Canvas',
    onSelect, onMove, onResize, onCreate, onCameraChange, onHover, onActivate, onReady, onError, onAssetProblem,
  }: Props = $props();

  let host: HTMLDivElement;
  let surface: HTMLCanvasElement;
  let renderer = $state('');
  let ready = $state(false);
  let failed = $state(false);
  let systemReduced = $state(false);

  // Not reactive on purpose: the Pixi objects are driven from effects and the frame loop.
  let app: Application | null = null;
  let world: WorldView | null = null;
  let assets: AssetCache | null = null;
  let retryTimer: ReturnType<typeof setInterval> | undefined;
  /** The longest one asset request may take, and how often the failed ones are looked at. */
  const ASSET_TIMEOUT_MS = 15_000;
  const ASSET_RETRY_EVERY_MS = 5_000;
  let overlay: OverlayView | null = null;
  let resolved = EMPTY_SCENE;
  let viewport: Size = { w: 0, h: 0 };
  let dpr = 1;
  let hover: string | null = null;
  let draft: Rect | null = null;
  let raf = 0;
  let needCull = false;
  // Milliseconds spent in the last frame, by part; for measurements.
  const timing = { cull: 0, overlay: 0, render: 0, frames: 0 };
  let culledWorld: Rect | null = null;
  let settleTimer: ReturnType<typeof setTimeout> | undefined;
  let tightenTimer: ReturnType<typeof setTimeout> | undefined;

  const palette = $derived(pixiColors(theme));
  const reduced = $derived(reducedMotion ?? systemReduced);

  function schedule(): void {
    if (!raf && app) raf = requestAnimationFrame(frame);
  }

  function frame(): void {
    raf = 0;
    if (!app || !world || !overlay) return;
    const now = performance.now();
    timing.cull = 0;
    if (needCull) {
      needCull = false;
      cullNow();
    }
    const t0 = performance.now();
    overlay.draw({
      camera, viewport, scene: resolved, selection, hover, touches, draft, indicator, palette, reducedMotion: reduced, now,
    });
    const t1 = performance.now();
    app.renderer.render(app.stage);
    timing.overlay = t1 - t0;
    timing.render = performance.now() - t1;
    timing.frames++;
    if (OverlayView.animating(touches, now, reduced)) schedule();
  }

  /** Hides what is outside the view, with half a viewport of margin so a slow pan does not show gaps. */
  function cullNow(): void {
    if (!world) return;
    const mx = viewport.w / 2;
    const my = viewport.h / 2;
    const view = { x: -mx, y: -my, width: viewport.w + 2 * mx, height: viewport.h + 2 * my };
    const t = performance.now();
    world.cull(view);
    timing.cull = performance.now() - t;
    culledWorld = rectToWorld(camera, { x: view.x, y: view.y, w: view.width, h: view.height });
  }

  /** Pan or zoom has settled: tighten the culling and sharpen text for the new zoom. */
  function settle(): void {
    clearTimeout(settleTimer);
    if (!world) return;
    cullNow();
    world.sharpenText(camera.zoom, dpr);
    schedule();
  }

  function settleSoon(): void {
    clearTimeout(settleTimer);
    settleTimer = setTimeout(() => {
      onCameraChange?.(camera, true);
      settle();
    }, duration.fast);
  }

  // ---- props -> pixi -------------------------------------------------------------------------------

  $effect(() => {
    const s = scene;
    if (!ready || !world) return;
    const r = resolveScene(s, resolved);
    resolved = r.scene;
    world.apply(r.scene, r.patch);
    // A node that appears, or moves, resizes or turns visible, may enter the view while the camera stays still.
    if (r.patch.added.length || r.patch.changed.length) needCull = true;
    schedule();
  });

  $effect(() => {
    const c = camera;
    if (!ready || !world) return;
    world.setCamera(c);
    if (!culledWorld || !containsRect(culledWorld, visibleWorldRect(c, viewport))) needCull = true;
    // Whoever moved the camera, tighten the culling once it stops (zooming in leaves the old, wider cull valid).
    clearTimeout(tightenTimer);
    tightenTimer = setTimeout(settle, duration.fast);
    schedule();
  });

  $effect(() => {
    // Anything the overlay draws from: a change only needs another frame.
    void [selection, touches, indicator, palette, reduced, tool];
    if (ready) schedule();
  });

  $effect(() => {
    const p = palette;
    if (!ready || !app) return;
    app.renderer.background.color = p.canvas;
    schedule();
  });

  // ---- input -> intents ----------------------------------------------------------------------------

  const env: Env = {
    camera: () => camera,
    selection: () => selection,
    tool: () => tool,
    viewport: () => viewport,
    hit: (x, y) => hitTest(resolved, x, y),
    selectionBox: () => selectionBox(camera, rectsOf(resolved, selection)),
  };

  const setCamera = (c: Camera, final: boolean) => {
    camera = c;
    onCameraChange?.(c, final);
    // Cull against the new transform, not the one the camera effect has not applied yet.
    if (final) {
      world?.setCamera(c);
      settle();
    }
  };

  const intents: Intents = {
    select: (ids, additive) => onSelect?.(ids, additive),
    move: (i) => onMove?.(i),
    resize: (i) => onResize?.(i),
    create: (i) => {
      draft = i.final ? null : { x: i.x, y: i.y, w: i.w, h: i.h };
      onCreate?.(i);
      schedule();
    },
    camera: setCamera,
    hover: (id) => {
      hover = id;
      onHover?.(id);
      schedule();
    },
    fit: (kind) => (kind === 'selection' ? fitSelection() : fitAll()),
  };

  const interaction = new Interaction(env, intents);

  /** Shows all the content. */
  export function fitAll(): void {
    setCamera(fitRect(contentBounds(resolved), viewport), true);
  }

  /** Shows the selection, or all the content when nothing is selected. */
  export function fitSelection(): void {
    const box = unionRects(rectsOf(resolved, selection));
    if (!box) fitAll();
    else setCamera(fitRect(box, viewport), true);
  }

  /** The world point at the centre of what the canvas shows. */
  export function centre(): { x: number; y: number } {
    return screenToWorld(camera, viewport.w / 2, viewport.h / 2);
  }

  /** Gives the keyboard back to the canvas (after an editor laid over it closes). */
  export function focus(): void {
    host?.focus({ preventScroll: true });
  }

  /** The box in world units of a node, or of all the content when no id is given; null when the node is not here. */
  export function captureBox(id?: string): Rect | null {
    return id === undefined ? contentBounds(resolved) : (rectsOf(resolved, [id])[0] ?? null);
  }

  /** The world in `box` at `scale` as a PNG, without the overlay; the view is drawn again afterwards. */
  export async function capture(box: Rect, scale: number): Promise<Blob> {
    if (!app || !world) throw new Error('The canvas is not ready.');
    // Every picture the view asked for is in the image (a missing one is drawn as its fill, as on the screen), unless one
    // takes too long: the capture then shows that node by its fill.
    await assets?.idle(ASSET_TIMEOUT_MS);
    if (!app || !world) throw new Error('The canvas is not ready.');
    const width = Math.ceil(box.w * scale);
    const height = Math.ceil(box.h * scale);
    let image: HTMLCanvasElement;
    try {
      image = world.renderRegion(app.renderer, box, scale, width, height);
    } finally {
      needCull = true; // the drawing showed everything: cull again for the view
      schedule();
    }
    return new Promise((resolve, reject) => image.toBlob((b) => (b ? resolve(b) : reject(new Error('The image could not be encoded.'))), 'image/png'));
  }

  /** What the canvas is drawing now; for measurements and tests. */
  export function stats(): { nodes: number; drawn: number; renderer: string; dpr: number; timing: typeof timing } {
    return { timing: { ...timing }, nodes: world?.nodeCount ?? 0, drawn: world?.countRenderable() ?? 0, renderer: app?.renderer.name ?? '', dpr };
  }

  function resize(): void {
    const r = host.getBoundingClientRect();
    viewport = { w: Math.max(1, Math.round(r.width)), h: Math.max(1, Math.round(r.height)) };
    app?.renderer.resize(viewport.w, viewport.h);
    needCull = true;
    schedule();
  }

  onMount(() => {
    let disposed = false;
    const detachInput = attachInput(host, {
      interaction,
      tool: () => tool,
      camera: () => camera,
      hit: (x, y) => hitTest(resolved, x, y),
      schedule,
      dropDraft: () => (draft = null),
      settleSoon,
      activate: (id) => onActivate?.(id),
    });

    const observer = new ResizeObserver(() => resize());
    observer.observe(host);

    let motion: MediaQueryList | undefined;
    let density: MediaQueryList | undefined;
    const onMotion = () => (systemReduced = !!motion?.matches);
    const onDensity = () => {
      dpr = window.devicePixelRatio || 1;
      if (app) app.renderer.resolution = dpr;
      resize();
      watchDensity();
    };
    function watchDensity(): void {
      density?.removeEventListener('change', onDensity);
      density = window.matchMedia?.(`(resolution: ${window.devicePixelRatio || 1}dppx)`);
      density?.addEventListener('change', onDensity);
    }
    motion = window.matchMedia?.('(prefers-reduced-motion: reduce)');
    onMotion();
    motion?.addEventListener('change', onMotion);
    dpr = window.devicePixelRatio || 1;
    watchDensity();

    // A web font that arrives after the first draw: draw the text again.
    // (`ready` can resolve before a face that the canvas has not used yet starts loading, so also listen for `loadingdone`.)
    const onFonts = () => {
      world?.refreshText();
      schedule();
    };
    void document.fonts?.ready.then(onFonts);
    document.fonts?.addEventListener('loadingdone', onFonts);

    void (async () => {
      const a = new Application();
      try {
        const r = host.getBoundingClientRect();
        viewport = { w: Math.max(1, Math.round(r.width)), h: Math.max(1, Math.round(r.height)) };
        await a.init({
          canvas: surface,
          width: viewport.w,
          height: viewport.h,
          resolution: dpr,
          autoDensity: true,
          antialias: true,
          backgroundColor: palette.canvas,
          // WebGL, and the 2D canvas renderer of Pixi when WebGL is not available.
          preference: ['webgl', 'canvas'],
          autoStart: false,
          sharedTicker: false,
        });
      } catch (err) {
        if (!disposed) {
          failed = true;
          onError?.(err);
        }
        try {
          a.destroy({ removeView: false }, { children: true });
        } catch {
          // init failed half way: nothing more to release
        }
        return;
      }
      if (disposed) {
        a.destroy({ removeView: false }, { children: true });
        return;
      }
      app = a;
      a.ticker.stop();
      // The images of image fills: fetched through the same origin (the dev proxy adds the token), decoded off the main thread.
      assets = new AssetCache({ fetch: (url) => fetch(url, { signal: AbortSignal.timeout(ASSET_TIMEOUT_MS) }), decode: (blob) => createImageBitmap(blob) });
      // An image that failed a while ago (the server was restarting, the image was not uploaded yet) is asked for again.
      retryTimer = setInterval(() => assets?.retryProblems(), ASSET_RETRY_EVERY_MS);
      assets.onProblem = (id, code) => onAssetProblem?.(id, code);
      assets.onChange((id) => {
        world?.repaintAsset(id);
        needCull = true;
        schedule();
      });
      world = new WorldView(assets);
      overlay = new OverlayView();
      a.stage.addChild(world.root, overlay.root);
      a.stage.eventMode = 'none';
      world.setCamera(camera);
      ready = true;
      resize(); // the observer may have fired during the async init, before the app existed
      renderer = a.renderer.name;
      onReady?.({ renderer });
    })();

    return () => {
      disposed = true;
      document.fonts?.removeEventListener('loadingdone', onFonts);
      detachInput();
      observer.disconnect();
      motion?.removeEventListener('change', onMotion);
      density?.removeEventListener('change', onDensity);
      clearTimeout(settleTimer);
      clearTimeout(tightenTimer);
      if (raf) cancelAnimationFrame(raf);
      raf = 0;
      ready = false;
      renderer = '';
      overlay?.destroy();
      world?.destroy();
      clearInterval(retryTimer);
      assets?.destroy();
      assets = null;
      app?.destroy({ removeView: false }, { children: true });
      app = null;
      world = null;
      overlay = null;
    };
  });
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div
  bind:this={host}
  class="canvas-host"
  role="application"
  aria-label={label}
  tabindex="0"
  data-renderer={renderer || undefined}
>
  <canvas bind:this={surface} class="canvas-surface"></canvas>
  {#if failed}
    <p class="canvas-failed">The canvas could not start in this browser.</p>
  {/if}
</div>

<style>
  .canvas-host {
    position: relative;
    width: 100%;
    height: 100%;
    overflow: hidden;
    touch-action: none;
    user-select: none;
    outline: none;
    background: var(--mf-canvas);
  }
  .canvas-surface {
    display: block;
  }
  .canvas-failed {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    margin: 0;
    color: var(--mf-text-muted);
    font-size: var(--mf-font-size-body);
  }
</style>
