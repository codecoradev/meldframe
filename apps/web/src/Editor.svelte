<!--
  The editor page: the canvas over the document the app layer holds, with the connection state. It only renders
  the app state and dispatches actions (thin UI, ADR-0008). A gesture shows a preview while it goes on and becomes
  one action when it ends. The chrome around it is the Layout (packages/web-chrome).
-->
<script lang="ts">
  import { onDestroy, onMount, untrack } from 'svelte';
  import type { Action, AppState } from '@meldframe/protocol';
  import { HOME, formatZoom, type Camera } from '@meldframe/web-canvas/camera';
  import type { Engine } from '@meldframe/web-link/engine';
  import type { CreateIntent, MoveIntent, ResizeIntent } from '@meldframe/web-canvas/interaction';
  import Canvas from './canvas/Canvas.svelte';
  import { createActionFor, keyAction, ownsKeys, worldOrigin, moveActionsFor, previewScene, resizeActionFor, resizedBox, toolMode, type Preview } from '@meldframe/web-document/edit';
  import { commitText, editKeyTarget, textBoxStyle, textTarget } from '@meldframe/web-document/textedit';
  import TextEditor from './TextEditor.svelte';
  import Layout from '@meldframe/web-chrome/Layout.svelte';
  import Section from '@meldframe/web-chrome/Section.svelte';
  import { layoutKey } from '@meldframe/web-chrome/keys';
  import LayersPanel from '@meldframe/web-panels/LayersPanel.svelte';
  import PropertiesPanel from '@meldframe/web-panels/PropertiesPanel.svelte';
  import VariablesPanel from '@meldframe/web-panels/VariablesPanel.svelte';
  import { layerRows } from '@meldframe/web-document/layers';
  import { noticeFromEvents } from '@meldframe/web-document/notices';
  import { presetAction } from '@meldframe/web-document/presets';
  import { type ExportScale, exportFileName, planExport } from '@meldframe/web-document/export';
  import { addTouch, type Touched } from '@meldframe/web-document/touches';
  import { serveCapture, type Req } from '@meldframe/web-document/capture-serve';
  import { duration, pixiColors } from '@meldframe/web-tokens/tokens';
  import { loadTheme } from '@meldframe/web-tokens/theme';

  interface Props {
    engine: Engine;
    /** The file's name, shown at the top of the layers panel. */
    file: string;
    /** The actor of this page: its own changes carry no label in the layers panel. */
    actor: string;
    /** Who last changed each node when the file was loaded; live changes are in the app state. */
    attribution?: Record<string, { actor: string; seq: number }>;
    /** Where the ephemeral touch frames of other actors come from; returns the way to stop listening. */
    onTouches?: (fn: (t: { actor: string; nodeIds: string[] }) => void) => () => void;
    /** Capture requests of agents: where they come from, and how to tell the server this editor cannot serve one. */
    captures?: { subscribe: (fn: (r: Req) => void) => () => void; fail: (requestId: string, code: string, message: string) => void };
  }
  let { engine, file, actor, attribution = {}, onTouches, captures }: Props = $props();

  // Snapshots are replaced as a whole, never edited: no deep proxy over the nodes.
  let snap = $state.raw(untrack(() => engine.snapshot()));
  let camera = $state<Camera>({ ...HOME });
  let canvas = $state<ReturnType<typeof Canvas>>();
  const theme = loadTheme();

  // The scene comes from the Rust render step with the nodes: a change of selection or connection reuses both, and the
  // engine reads them once per change of the document.
  const nodes = $derived(snap.nodes);
  const scene = $derived(snap.scene);

  // What a gesture in progress shows; the file changes only when it ends (one action, one undo step).
  let preview = $state<Preview | null>(null);
  const shown = $derived(previewScene(scene, preview));
  let helpOpen = $state(false);
  const label = (c: AppState['connection']): string =>
    ({ live: 'Connected', connecting: 'Connecting', reconnecting: 'Reconnecting', offline: 'Offline', halted: 'Out of sync' })[c.type];

  const dispatch = (a: Action) => void engine.dispatch(a);

  // The layers: what the server stamped when the file loaded, then what the app saw change since.
  const actors = $derived({ ...Object.fromEntries(Object.entries(attribution).map(([id, a]) => [id, a.actor])), ...snap.state.lastActor });
  const rows = $derived(layerRows(nodes, scene, snap.state.panels.collapsed, snap.state.selection, actors));

  // The touch rings of other actors: a ring flashes on the nodes they just changed and fades.
  let touches = $state.raw<Touched[]>([]);
  onMount(() =>
    onTouches?.((t) => {
      const c = pixiColors(theme);
      touches = addTouch(touches, t, performance.now(), {
        self: actor,
        palette: { person: c.person, agents: [c['agent-1'], c['agent-2'], c['agent-3']] },
        maxAgeMs: Math.max(duration['touch-ring'], duration['reduced-motion-hold']),
      });
    }),
  );

  // An agent asks for a PNG of a node: the canvas draws it offscreen and the page uploads it (see capture.ts).
  const waitForSeq = (min: number, timeoutMs: number) =>
    new Promise<boolean>((resolve) => {
      if (engine.snapshot().state.confirmedSeq >= min) return resolve(true);
      const off = engine.subscribe((s) => {
        if (s.state.confirmedSeq < min) return;
        off();
        clearTimeout(timer);
        resolve(true);
      });
      const timer = setTimeout(() => (off(), resolve(false)), timeoutMs);
    });
  onMount(() =>
    captures?.subscribe((r) =>
      void serveCapture(r, {
        box: (id) => canvas?.captureBox(id) ?? null,
        render: (box, scale) => canvas!.capture(box, scale),
        seq: () => engine.snapshot().state.confirmedSeq,
        whenSeq: waitForSeq,
        upload: async (id, headers, body) => (await fetch(`/captures/${encodeURIComponent(id)}`, { method: 'POST', headers, body })).status,
        fail: captures.fail,
      }),
    ),
  );

  // The one selected frame: PNG export of it (E4.3, drawn offscreen like a capture, then saved by the browser) and a
  // variant of it (E5.4, the `makeVariant` action).
  const exportable = $derived.by(() => {
    const id = snap.state.selection.length === 1 ? snap.state.selection[0] : undefined;
    const n = id ? nodes.find((x) => x.id === id) : undefined;
    return n && n.type === 'frame' ? n : null;
  });
  async function exportPng(scale: ExportScale) {
    const n = exportable;
    if (!canvas || !n) return;
    const box = canvas.captureBox(n.id);
    const plan = planExport(box, scale);
    if (!plan.ok || !box) return showNotice(plan.ok || plan.code !== 'too_large' ? 'There is nothing to export.' : 'The image would be larger than 4096 x 4096 pixels in area.');
    try {
      const url = URL.createObjectURL(await canvas.capture(box, scale));
      const a = document.createElement('a');
      a.href = url;
      a.download = exportFileName(n.name, n.id, scale);
      document.body.append(a);
      a.click();
      a.remove();
      // Some browsers read the URL after the click returns: keep it a while.
      setTimeout(() => URL.revokeObjectURL(url), 60_000);
    } catch {
      showNotice('The image could not be drawn.');
    }
  }

  // A refusal is shown for a few seconds, then goes away; a later snapshot does not cancel that (the timer is not tied to the effect).
  let notice = $state<string | null>(null);
  let noticeTimer: ReturnType<typeof setTimeout> | undefined;
  function showNotice(text: string) {
    notice = text;
    clearTimeout(noticeTimer);
    noticeTimer = setTimeout(() => (notice = null), 6000);
  }
  $effect(() => {
    const text = noticeFromEvents(snap.events);
    if (text) untrack(() => showNotice(text));
  });
  onDestroy(() => clearTimeout(noticeTimer));

  // The text being edited: the node and the text it had when the field opened (an unchanged commit makes no step).
  let editing = $state<{ id: string; before: string } | null>(null);
  const editedNode = $derived(editing ? textTarget(editing.id, nodes) : null);
  // The box comes from the nodes (not from what the canvas drew last), so it follows a change of the node at once.
  const editBox = $derived.by(() => {
    const n = editedNode;
    if (!n) return null;
    const o = worldOrigin(nodes, n.parentId);
    return textBoxStyle(camera, { x: o.x + n.x, y: o.y + n.y, w: n.w, h: n.h }, n.fontSize);
  });
  // The node is gone (someone deleted it): the field closes with nothing to commit.
  $effect(() => {
    if (editing && !editedNode) editing = null;
  });
  function startEdit(id: string) {
    const n = textTarget(id, nodes);
    if (!n) return;
    dispatch({ type: 'select', ids: [id] });
    editing = { id, before: n.text };
  }
  function endEdit(text: string | null) {
    const e = editing;
    editing = null;
    if (e && text !== null) {
      const a = commitText(e.id, e.before, text);
      if (a) dispatch(a);
    }
    canvas?.focus();
  }

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
  // An image that is missing or cannot be loaded is said once, with its code; the node shows its fill meanwhile.
  // (a plain Set: it is read only here, not by the template, so it needs no reactivity)
  // eslint-disable-next-line svelte/prefer-svelte-reactivity
  const shownAssets = new Set<string>();
  function onAssetProblem(id: string, code: string) {
    if (shownAssets.has(id)) return;
    shownAssets.add(id);
    showNotice(`An image could not be loaded (${code}).`);
  }
  $effect(() => {
    if (ready && !fitted && nodes.length) {
      fitted = true;
      canvas?.fitAll();
    }
  });

  function onMove(i: MoveIntent) {
    if (i.cancelled || i.final) preview = null;
    else preview = { kind: 'move', ids: i.ids, dx: i.dx, dy: i.dy };
    for (const a of moveActionsFor(i, snap.state.selection, nodes)) dispatch(a);
  }

  /** One node is resized at a time: the handles of a selection of several nodes do nothing yet. */
  function onResize(i: ResizeIntent) {
    if (i.cancelled || i.final) preview = null;
    const n = i.ids.length === 1 ? nodes.find((x) => x.id === i.ids[0]) : undefined;
    if (!n) return; // a node that is gone meanwhile (a change of someone else) ends the gesture quietly
    if (!i.cancelled && !i.final) preview = { kind: 'box', id: n.id, box: resizedBox(n, i.handle, i.dx, i.dy) };
    const a = resizeActionFor(i, nodes);
    if (a) dispatch(a);
  }

  /** The canvas draws the box while it is dragged; the node is made when the gesture ends. */
  function onCreate(i: CreateIntent) {
    const a = createActionFor(i, nodes);
    if (a) dispatch(a);
  }

  /** Keys of the editor (tools, delete, undo, redo), unless the person is typing in a field. */
  function onKey(e: KeyboardEvent) {
    if (e.repeat || e.isComposing) return; // a held key must not undo a step per repeat; a composition is not a command
    if (ownsKeys(e.target as HTMLElement | null, e)) return;
    const key = layoutKey(e);
    if (key) {
      e.preventDefault();
      if (key === 'help') helpOpen = !helpOpen;
      else dispatch({ type: 'togglePanel', panel: key === 'layers' ? 'layers' : 'properties' });
      return;
    }
    const edit = editKeyTarget(e, snap.state.selection, nodes, e.target as HTMLElement | null);
    if (edit) {
      e.preventDefault();
      startEdit(edit);
      return;
    }
    const a = keyAction(e, snap.state.selection);
    if (!a) return;
    e.preventDefault();
    dispatch(a);
  }

  /** Shift adds to the selection or takes out of it; a plain click replaces it. */
  function onSelect(ids: string[], additive: boolean) {
    if (additive) for (const id of ids) dispatch({ type: 'toggleSelect', id });
    else dispatch({ type: 'select', ids });
  }
</script>

<svelte:window onkeydown={onKey} />

<Layout
  fileName={file}
  tool={snap.state.tool}
  onTool={(tool) => dispatch({ type: 'setTool', tool })}
  layersOpen={snap.state.panels.layers}
  inspectorOpen={snap.state.panels.properties}
  onToggleLayers={() => dispatch({ type: 'togglePanel', panel: 'layers' })}
  onToggleInspector={() => dispatch({ type: 'togglePanel', panel: 'properties' })}
  canUndo={snap.state.history.canUndo}
  canRedo={snap.state.history.canRedo}
  onUndo={() => dispatch({ type: 'undo' })}
  onRedo={() => dispatch({ type: 'redo' })}
  onPreset={(p) => canvas && dispatch(presetAction(p, canvas.centre()))}
  canExport={exportable !== null}
  onExport={(s) => void exportPng(s)}
  canVariant={exportable !== null}
  onVariant={() => exportable && dispatch({ type: 'makeVariant', id: exportable.id })}
  zoomText={formatZoom(camera.zoom)}
  connection={{ state: snap.state.connection.type, text: label(snap.state.connection) }}
  {helpOpen}
  onHelp={(open) => (helpOpen = open)}
>
  {#snippet canvasLayer()}
    <Canvas
      bind:this={canvas}
      scene={shown}
      selection={snap.state.selection}
      tool={toolMode(snap.state.tool)}
      bind:camera
      {theme}
      {touches}
      label={`File ${file}`}
      {onSelect}
      {onMove}
      {onResize}
      {onCreate}
      onActivate={startEdit}
      {onReady}
      onError={(e) => (canvasError = e instanceof Error ? e.message : String(e))}
      {onAssetProblem}
    />
    {#if editing && editBox}
      <TextEditor text={editing.before} box={editBox} onCommit={endEdit} onCancel={() => endEdit(null)} />
    {/if}
    {#if notice}
      <div role="status" data-testid="notice" class="absolute bottom-24 left-1/2 max-w-80 -translate-x-1/2 rounded-md bg-warning-bg px-4 py-2 text-small text-text-strong shadow-float">{notice}</div>
    {/if}
    {#if canvasError}
      <div role="alert" data-testid="canvas-error" class="absolute inset-x-0 top-4 mx-auto w-fit rounded-md bg-danger-bg px-4 py-2 text-small text-danger">
        The canvas could not start: {canvasError}
      </div>
    {/if}
    {#if snap.state.connection.type === 'halted'}
      <div role="alert" class="absolute inset-x-0 top-4 mx-auto w-fit rounded-md bg-danger-bg px-4 py-2 text-small text-danger">
        This page and the server no longer agree about the file.
        <button class="ml-2 underline" onclick={() => location.reload()}>Reload</button>
      </div>
    {/if}
  {/snippet}
  {#snippet layersPanel()}
    <LayersPanel
      {rows}
      self={actor}
      onSelect={(id, additive) => (additive ? dispatch({ type: 'toggleSelect', id }) : dispatch({ type: 'select', ids: [id] }))}
      onToggle={(id) => dispatch({ type: 'toggleCollapsed', id })}
    />
  {/snippet}
  {#snippet designPanel()}
    <Section title="Variables" open={snap.state.panels.variables} onToggle={() => dispatch({ type: 'togglePanel', panel: 'variables' })} toggleId="toggle-variables">
      <VariablesPanel panel={snap.state.variables} target={snap.state.selection.length === 1 ? (snap.state.selection[0] ?? null) : null} {dispatch} />
    </Section>
    <PropertiesPanel properties={snap.state.properties} picker={snap.state.picker ?? null} selected={snap.state.selection.length} {dispatch} />
  {/snippet}
</Layout>
