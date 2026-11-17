<!--
  The editor page: the canvas over the document the app layer holds, with the connection state. It only renders
  the app state and dispatches actions (thin UI, ADR-0008). The panels and the tools arrive in the next slices.
-->
<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import type { Action, AppState } from '@meldframe/protocol';
  import { HOME, formatZoom, type Camera } from '@meldframe/web-canvas/camera';
  import type { Engine } from '@meldframe/web-link/engine';
  import Canvas from './canvas/Canvas.svelte';
  import { sceneFromNodes } from './scene';
  import { loadTheme } from './theme';

  interface Props {
    engine: Engine;
    /** The file's name, shown in the corner. */
    file: string;
  }
  let { engine, file }: Props = $props();

  // Snapshots are replaced as a whole, never edited: no deep proxy over the nodes.
  let snap = $state.raw(untrack(() => engine.snapshot()));
  let camera = $state<Camera>({ ...HOME });
  let canvas = $state<ReturnType<typeof Canvas>>();
  const theme = loadTheme();

  // The scene depends on the nodes only: a change of selection or connection reuses the same array, so nothing is rebuilt.
  const nodes = $derived(snap.nodes);
  const scene = $derived(sceneFromNodes(nodes));
  const label = (c: AppState['connection']): string =>
    ({ live: 'Connected', connecting: 'Connecting', reconnecting: 'Reconnecting', offline: 'Offline', halted: 'Out of sync' })[c.type];

  const dispatch = (a: Action) => void engine.dispatch(a);

  // `mount` runs this after the shell may have started: read what the engine holds now, then follow it.
  onMount(() => {
    snap = engine.snapshot();
    return engine.subscribe((s) => (snap = s));
  });

  // Show the content in full the first time there is some and the canvas is ready: a file that opens with content,
  // and one that opens empty and gets its first nodes over the socket. After that the camera is the person's.
  let canvasError = $state<string | null>(null);
  let ready = $state(false);
  let fitted = false;
  const onReady = () => (ready = true);
  $effect(() => {
    if (ready && !fitted && nodes.length) {
      fitted = true;
      canvas?.fitAll();
    }
  });

  /** Shift adds to the selection or takes out of it; a plain click replaces it. */
  function onSelect(ids: string[], additive: boolean) {
    if (additive) for (const id of ids) dispatch({ type: 'toggleSelect', id });
    else dispatch({ type: 'select', ids });
  }
</script>

<div class="fixed inset-0 bg-canvas font-sans text-body text-text">
  <Canvas
    bind:this={canvas}
    {scene}
    selection={snap.state.selection}
    bind:camera
    {theme}
    label={`File ${file}`}
    {onSelect}
    {onReady}
    onError={(e) => (canvasError = e instanceof Error ? e.message : String(e))}
  />
  {#if canvasError}
    <div role="alert" data-testid="canvas-error" class="absolute inset-x-0 top-4 mx-auto w-fit rounded-md bg-danger-bg px-4 py-2 text-small text-danger">
      The canvas could not start: {canvasError}
    </div>
  {/if}
  <div class="pointer-events-none absolute top-4 left-4 rounded-md bg-surface/90 px-3 py-1.5 text-small text-text-muted">{file}</div>
  <div class="pointer-events-none absolute right-5 bottom-7 flex items-center gap-3 text-small text-text-muted">
    <output data-testid="connection" data-state={snap.state.connection.type}>{label(snap.state.connection)}</output>
    <output class="font-mono">{formatZoom(camera.zoom)}</output>
  </div>
  {#if snap.state.connection.type === 'halted'}
    <div role="alert" class="absolute inset-x-0 top-4 mx-auto w-fit rounded-md bg-danger-bg px-4 py-2 text-small text-danger">
      This page and the server no longer agree about the file.
      <button class="ml-2 underline" onclick={() => location.reload()}>Reload</button>
    </div>
  {/if}
</div>
