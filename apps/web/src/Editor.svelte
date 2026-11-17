<!--
  The editor page: the canvas over the document the app layer holds, with the connection state. It only renders
  the app state and dispatches actions (thin UI, ADR-0008). A gesture shows a preview while it goes on and becomes
  one action when it ends. The panels and the tool bar arrive in the next slice.
-->
<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import type { Action, AppState } from '@meldframe/protocol';
  import { HOME, formatZoom, type Camera } from '@meldframe/web-canvas/camera';
  import type { Engine } from '@meldframe/web-link/engine';
  import type { CreateIntent, MoveIntent, ResizeIntent } from '@meldframe/web-canvas/interaction';
  import Canvas from './canvas/Canvas.svelte';
  import { createActionFor, keyAction, worldOrigin, moveActionsFor, previewScene, resizeActionFor, resizedBox, toolMode, type Preview } from '@meldframe/web-document/edit';
  import { sceneFromNodes } from '@meldframe/web-document/scene';
  import { commitText, editKeyTarget, textBoxStyle, textTarget } from '@meldframe/web-document/textedit';
  import TextEditor from './TextEditor.svelte';
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

  // What a gesture in progress shows; the file changes only when it ends (one action, one undo step).
  let preview = $state<Preview | null>(null);
  const shown = $derived(previewScene(scene, preview));
  const toolLabel = $derived({ select: 'Select', frame: 'Frame', rectangle: 'Rectangle', text: 'Text' }[snap.state.tool]);
  const label = (c: AppState['connection']): string =>
    ({ live: 'Connected', connecting: 'Connecting', reconnecting: 'Reconnecting', offline: 'Offline', halted: 'Out of sync' })[c.type];

  const dispatch = (a: Action) => void engine.dispatch(a);

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
    const el = e.target as HTMLElement | null;
    if (el && (el.isContentEditable || /^(input|textarea|select)$/i.test(el.tagName))) return;
    const edit = editKeyTarget(e, snap.state.selection, nodes);
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

<div class="fixed inset-0 bg-canvas font-sans text-body text-text">
  <Canvas
    bind:this={canvas}
    scene={shown}
    selection={snap.state.selection}
    tool={toolMode(snap.state.tool)}
    bind:camera
    {theme}
    label={`File ${file}`}
    {onSelect}
    {onMove}
    {onResize}
    {onCreate}
    onActivate={startEdit}
    {onReady}
    onError={(e) => (canvasError = e instanceof Error ? e.message : String(e))}
  />
  {#if editing && editBox}
    <TextEditor text={editing.before} box={editBox} onCommit={endEdit} onCancel={() => endEdit(null)} />
  {/if}
  {#if canvasError}
    <div role="alert" data-testid="canvas-error" class="absolute inset-x-0 top-4 mx-auto w-fit rounded-md bg-danger-bg px-4 py-2 text-small text-danger">
      The canvas could not start: {canvasError}
    </div>
  {/if}
  <div class="pointer-events-none absolute top-4 left-4 rounded-md bg-surface/90 px-3 py-1.5 text-small text-text-muted">{file}</div>
  <div class="pointer-events-none absolute right-5 bottom-7 flex items-center gap-3 text-small text-text-muted">
    <output data-testid="tool" data-tool={snap.state.tool}>{toolLabel}</output>
    <output data-testid="connection" data-state={snap.state.connection.type}>{label(snap.state.connection)}</output>
    <output class="font-mono" data-testid="zoom">{formatZoom(camera.zoom)}</output>
  </div>
  {#if snap.state.connection.type === 'halted'}
    <div role="alert" class="absolute inset-x-0 top-4 mx-auto w-fit rounded-md bg-danger-bg px-4 py-2 text-small text-danger">
      This page and the server no longer agree about the file.
      <button class="ml-2 underline" onclick={() => location.reload()}>Reload</button>
    </div>
  {/if}
</div>
