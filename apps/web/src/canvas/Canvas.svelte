<!--
  The canvas of the editor: draws the scene it is given with PixiJS and reports what the user does as
  intents. It holds no document state and applies no edit: a move or resize is reported in world units
  and the host decides what it means (U1, U2). Colours come from the tokens module; the colours of the
  nodes themselves are document data inside the scene.

  Everything but the markup is the driver (driver.ts), shared with the React copy (Canvas.tsx, E7.3 S11): this file hands it the
  props and the callbacks, the element and the surface, and shows what it reports (the renderer's name, a failed start).
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { HOME, type Camera } from '@meldframe/web-canvas/camera';
  import type { Rect } from '@meldframe/web-canvas/geometry';
  import type { CreateIntent, MoveIntent, ResizeIntent, ToolMode } from '@meldframe/web-canvas/interaction';
  import type { Scene } from '@meldframe/web-canvas/scene';
  import type { Theme } from '@meldframe/web-tokens/tokens';
  import type { Touch } from './overlayview';
  import { CanvasDriver } from './driver';
  import { browserPlatform } from './platform';
  import './canvas.css';

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
  let failed = $state(false);

  const driver = new CanvasDriver(browserPlatform());
  // Each callback is read when the driver calls it, so a new prop replaces the old one.
  driver.events = {
    onSelect: (ids, additive) => onSelect?.(ids, additive),
    onMove: (i) => onMove?.(i),
    onResize: (i) => onResize?.(i),
    onCreate: (i) => onCreate?.(i),
    onCameraChange: (c, final) => {
      camera = c;
      onCameraChange?.(c, final);
    },
    onHover: (id) => onHover?.(id),
    onActivate: (id) => onActivate?.(id),
    onReady: (info) => onReady?.(info),
    onError: (e) => onError?.(e),
    onAssetProblem: (id, code) => onAssetProblem?.(id, code),
    onStatus: (s) => {
      renderer = s.renderer;
      failed = s.failed;
    },
  };

  $effect(() => {
    driver.setProps({ scene, selection, camera: { x: camera.x, y: camera.y, zoom: camera.zoom }, touches, indicator, tool, theme, reducedMotion });
  });

  onMount(() => driver.mount(host, surface));

  /** Shows all the content. */
  export function fitAll(): void {
    driver.fitAll();
  }

  /** Shows the selection, or all the content when nothing is selected. */
  export function fitSelection(): void {
    driver.fitSelection();
  }

  /** The world point at the centre of what the canvas shows. */
  export function centre(): { x: number; y: number } {
    return driver.centre();
  }

  /** Gives the keyboard back to the canvas (after an editor laid over it closes). */
  export function focus(): void {
    driver.focus();
  }

  /** The box in world units of a node, or of all the content when no id is given, with the band of a border that lies outside; null when the node is not here. */
  export function captureBox(id?: string): Rect | null {
    return driver.captureBox(id);
  }

  /** The world in `box` at `scale` as a PNG, without the overlay; the view is drawn again afterwards. */
  export function capture(box: Rect, scale: number): Promise<Blob> {
    return driver.capture(box, scale);
  }

  /** What the canvas is drawing now; for measurements and tests. */
  export function stats(): ReturnType<CanvasDriver['stats']> {
    return driver.stats();
  }
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
