<!--
  The canvas of the editor: draws the scene it is given with PixiJS and reports what the user does as
  intents. It holds no document state and applies no edit: a move or resize is reported in world units
  and the host decides what it means (U1, U2). Colours come from the tokens module; the colours of the
  nodes themselves are document data inside the scene.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { Application } from 'pixi.js';
  import { HOME, fitRect, rectToWorld, visibleWorldRect, type Camera } from './camera';
  import { containsRect, unionRects, type Rect, type Size } from './geometry';
  import {
    Interaction, type CreateIntent, type Env, type Intents, type MoveIntent, type Mods, type ResizeIntent, type ToolMode,
  } from './interaction';
  import { OverlayView, type Touch } from './overlayview';
  import { selectionBox } from './overlay';
  import { EMPTY_SCENE, contentBounds, hitTest, rectsOf, resolveScene, type Scene } from './scene';
  import { WorldView } from './stage';
  import { duration, pixiColors, type Theme } from '../tokens';

  interface Props {
    scene: Scene;
    /** Ids drawn as selected. */
    selection?: readonly string[];
    /** The camera; the canvas changes it for pan, zoom and fit, and says so through `onCameraChange`. */
    camera?: Camera;
    touches?: readonly Touch[];
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
    /** The renderer that started, or the error when none could. */
    onReady?: (info: { renderer: string }) => void;
    onError?: (error: unknown) => void;
  }

  let {
    scene,
    selection = [],
    camera = $bindable({ ...HOME }),
    touches = [],
    tool = 'move',
    theme = 'dark',
    reducedMotion,
    label = 'Canvas',
    onSelect, onMove, onResize, onCreate, onCameraChange, onHover, onReady, onError,
  }: Props = $props();

  interface HostEvents {
    pointerdown: PointerEvent;
    pointermove: PointerEvent;
    pointerup: PointerEvent;
    pointercancel: PointerEvent;
    lostpointercapture: PointerEvent;
    pointerleave: PointerEvent;
    wheel: WheelEvent;
    keydown: KeyboardEvent;
    keyup: KeyboardEvent;
    blur: FocusEvent;
    contextmenu: MouseEvent;
  }

  let host: HTMLDivElement;
  let surface: HTMLCanvasElement;
  let renderer = $state('');
  let ready = $state(false);
  let failed = $state(false);
  let systemReduced = $state(false);

  // Not reactive on purpose: the Pixi objects are driven from effects and the frame loop.
  let app: Application | null = null;
  let world: WorldView | null = null;
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
      camera, viewport, scene: resolved, selection, hover, touches, draft, palette, reducedMotion: reduced, now,
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
    if (r.patch.added.length) needCull = true;
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
    void [selection, touches, palette, reduced, tool];
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
    if (final) settle();
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

  /** What the canvas is drawing now; for measurements and tests. */
  export function stats(): { nodes: number; drawn: number; renderer: string; dpr: number; timing: typeof timing } {
    return { timing: { ...timing }, nodes: world?.nodeCount ?? 0, drawn: world?.countRenderable() ?? 0, renderer: app?.renderer.name ?? '', dpr };
  }

  const modsOf = (e: { shiftKey: boolean; altKey: boolean; ctrlKey: boolean; metaKey: boolean }): Mods =>
    ({ shift: e.shiftKey, alt: e.altKey, ctrl: e.ctrlKey, meta: e.metaKey });

  function pointer(e: PointerEvent) {
    const r = host.getBoundingClientRect();
    return { id: e.pointerId, x: e.clientX - r.left, y: e.clientY - r.top, button: e.button, pointerType: e.pointerType, mods: modsOf(e) };
  }

  const refreshCursor = () => {
    host.style.cursor = interaction.cursor;
  };

  function resize(): void {
    const r = host.getBoundingClientRect();
    viewport = { w: Math.max(1, Math.round(r.width)), h: Math.max(1, Math.round(r.height)) };
    app?.renderer.resize(viewport.w, viewport.h);
    needCull = true;
    schedule();
  }

  onMount(() => {
    let disposed = false;
    const ac = new AbortController();
    const on = <K extends keyof HostEvents>(type: K, fn: (e: HostEvents[K]) => void, opts?: { passive?: boolean }) =>
      host.addEventListener(type, fn as (e: Event) => void, { ...opts, signal: ac.signal });

    on('pointerdown', (e) => {
      host.focus({ preventScroll: true });
      try {
        host.setPointerCapture(e.pointerId);
      } catch {
        // a synthetic or already-ended pointer cannot be captured: the gesture still works inside the element
      }
      interaction.down(pointer(e));
      refreshCursor();
      schedule();
    });
    on('pointermove', (e) => {
      interaction.move(pointer(e));
      refreshCursor();
      schedule();
    });
    on('pointerup', (e) => {
      interaction.up(pointer(e));
      refreshCursor();
      schedule();
    });
    on('pointercancel', () => {
      interaction.cancel();
      draft = null;
      refreshCursor();
      schedule();
    });
    on('lostpointercapture', () => {
      interaction.cancel();
      draft = null;
      refreshCursor();
    });
    on('pointerleave', () => interaction.leave());
    on('wheel', (e) => {
      e.preventDefault();
      const r = host.getBoundingClientRect();
      interaction.wheel({ x: e.clientX - r.left, y: e.clientY - r.top, deltaX: e.deltaX, deltaY: e.deltaY, deltaMode: e.deltaMode, ctrlKey: e.ctrlKey, metaKey: e.metaKey });
      settleSoon();
    }, { passive: false });
    const key = (down: boolean) => (e: KeyboardEvent) => {
      if (interaction.key({ key: e.key, code: e.code, down, mods: modsOf(e) })) e.preventDefault();
      draft = interaction.cursor === 'crosshair' ? draft : null;
      refreshCursor();
      schedule();
    };
    on('keydown', key(true));
    on('keyup', key(false));
    on('blur', () => {
      interaction.key({ key: ' ', down: false, mods: modsOf({ shiftKey: false, altKey: false, ctrlKey: false, metaKey: false }) });
      refreshCursor();
    });
    on('contextmenu', (e) => e.preventDefault());

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
    void document.fonts?.ready.then(() => {
      world?.refreshText();
      schedule();
    });

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
      world = new WorldView();
      overlay = new OverlayView();
      a.stage.addChild(world.root, overlay.root);
      a.stage.eventMode = 'none';
      world.setCamera(camera);
      ready = true;
      renderer = a.renderer.name;
      onReady?.({ renderer });
    })();

    return () => {
      disposed = true;
      ac.abort();
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
