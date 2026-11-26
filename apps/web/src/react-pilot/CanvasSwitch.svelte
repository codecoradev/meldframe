<!--
  The temporary switch of the canvas (docs/react-web-plan.md, section 7, slice S11): what Editor.svelte and the canvas harness
  import in place of Canvas.svelte. Without `/?react=canvas` it draws the Svelte canvas; with it, the React copy (canvas/react/Canvas.tsx)
  in an island, loaded on demand so that the editor's bundle does not carry React. If the React chunk does not load, the Svelte
  canvas (which is in the entry) is drawn, so that the person never has a page with no canvas. The host element draws nothing of
  its own (`display: contents`). It goes with the switch when the last slice removes the Svelte shell. `/?react=canvas,strict` (development
  only) draws the React copy inside React's strict mode.

  Two things that the Svelte file does through the framework are done here by hand: `bind:camera` (the island gets the camera as a
  plain prop and a callback, and the callback sets the bound camera), and the methods the editor calls through `bind:this`
  (fitAll, fitSelection, centre, focus, captureBox, capture, stats), which go to whichever canvas is drawn. Before the React chunk has
  loaded no canvas is drawn, so a call answers what a canvas that has not started answers (nothing, the origin, no box, a refused capture).
-->
<script lang="ts">
  import type { ComponentProps } from 'svelte';
  import { HOME, type Camera } from '@meldframe/web-canvas/camera';
  import type { Rect } from '@meldframe/web-canvas/geometry';
  import Canvas from '../canvas/Canvas.svelte';
  import type { CanvasHandle } from '../canvas/driver';
  import { reactFor } from './pilot';

  let { camera = $bindable({ ...HOME }), ...props }: ComponentProps<typeof Canvas> = $props();
  // The React side, island helper included, loads on demand: the editor's bundle does not carry react-dom.
  const react = reactFor('canvas', location.search)
    ? Promise.all([import('@meldframe/web-chrome/react/island'), import('../canvas/react/Canvas')]).then(([i, m]) => ({ island: i.island, component: m.Canvas }))
    : null;
  const strict = import.meta.env.DEV && reactFor('strict', location.search);

  let svelte = $state.raw<ReturnType<typeof Canvas>>(); // the Svelte canvas, when it is the one drawn
  let handle: CanvasHandle | null = null; // the React canvas, when it is the one drawn
  const setHandle = (h: CanvasHandle | null) => {
    handle = h;
  };
  const target = (): CanvasHandle | undefined => svelte ?? handle ?? undefined;
  const onCameraChange = (c: Camera, final: boolean) => {
    camera = c;
    props.onCameraChange?.(c, final);
  };

  /** Shows all the content. */
  export function fitAll(): void {
    target()?.fitAll();
  }
  /** Shows the selection, or all the content when nothing is selected. */
  export function fitSelection(): void {
    target()?.fitSelection();
  }
  /** The world point at the centre of what the canvas shows. */
  export function centre(): { x: number; y: number } {
    return target()?.centre() ?? { x: 0, y: 0 };
  }
  /** Gives the keyboard back to the canvas (after an editor laid over it closes). */
  export function focus(): void {
    target()?.focus();
  }
  /** The box in world units of a node, or of all the content when no id is given; null when the node is not here. */
  export function captureBox(id?: string): Rect | null {
    return target()?.captureBox(id) ?? null;
  }
  /** The world in `box` at `scale` as a PNG, without the overlay. */
  export function capture(box: Rect, scale: number): Promise<Blob> {
    const t = target();
    return t ? t.capture(box, scale) : Promise.reject(new Error('The canvas is not ready.'));
  }
  /** What the canvas is drawing now; for measurements and tests. */
  export function stats(): ReturnType<CanvasHandle['stats']> {
    return target()?.stats() ?? { nodes: 0, drawn: 0, renderer: '', dpr: 1, timing: { cull: 0, overlay: 0, render: 0, frames: 0 } };
  }
</script>

{#if react}
  {#await react then r}
    <div
      class="contents"
      use:r.island={{ component: r.component, strict, props: { ...props, camera: { x: camera.x, y: camera.y, zoom: camera.zoom }, onCameraChange, ref: setHandle } }}
    ></div>
  {:catch}
    <!-- the React chunk did not load: the Svelte canvas is in the entry, so the editor still has one -->
    <Canvas bind:this={svelte} bind:camera {...props} />
  {/await}
{:else}
  <Canvas bind:this={svelte} bind:camera {...props} />
{/if}
