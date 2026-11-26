<!--
  The editor page: the canvas over the document the app layer holds, with the connection state. It only renders
  the app state and dispatches actions (thin UI, ADR-0008). A gesture shows a preview while it goes on and becomes
  one action when it ends. The chrome around it is the Layout (packages/web-shell).
-->
<script lang="ts">
  import { onDestroy, onMount, untrack } from 'svelte';
  import type { Action } from '@meldframe/protocol';
  import { HOME, formatZoom, type Camera } from '@meldframe/web-canvas/camera';
  import type { Engine } from '@meldframe/web-link/engine';
  import type { LayoutSwitch } from '@meldframe/web-link/layoutswitch';
  import type { CreateIntent, MoveIntent, ResizeIntent } from '@meldframe/web-canvas/interaction';
  import Canvas from './canvas/Canvas.svelte';
  import { createActionFor, deleteAction, documentChanged, drawnBox, gestureGoing, keyAction, ownsKeys, worldOrigin, moveActionsFor, movePreviewFor, originLocked, previewScene, resizeActionFor, resizePreviewFor, toolMode, type Preview } from '@meldframe/web-document/edit';
  import { perFrame } from '@meldframe/web-document/coalesce';
  import { shouldRefit } from '@meldframe/web-document/fit';
  import { toggleWithNotice } from '@meldframe/web-link/layoutswitch';
  import { commitText, editKeyTarget, textBoxStyle, textTarget } from '@meldframe/web-document/textedit';
  import TextEditor from './TextEditor.svelte';
  import Layout from '@meldframe/web-shell/Layout.svelte';
  import Section from '@meldframe/web-chrome/Section.svelte';
  import BannerStack from '@meldframe/web-feedback/BannerStack.svelte';
  import type { Banner } from '@meldframe/web-feedback/banner';
  import Toast from '@meldframe/web-feedback/Toast.svelte';
  import NoFrames from '@meldframe/web-feedback/NoFrames.svelte';
  import { Toaster, type ToastView } from '@meldframe/web-feedback/toaster';
  import { layoutKey, paletteBlocked, readOverlays } from '@meldframe/web-shell/keys';
  import { bindParts, shellParts } from './react-pilot/shell'; // the palette and the shortcut sheet, Svelte or React with /?react=palette (E7.3, S4); the bind picker with /?react=variables (S5)
  import { zoomCamera } from '@meldframe/web-shell/zoom';
  import LayersPanel from './react-pilot/LayersPanelSwitch.svelte'; // LayersPanel.svelte of web-panels, or its React copy with /?react=layers (E7.3, S5)
  import PropertiesPanel from '@meldframe/web-panels/PropertiesPanel.svelte';
  import VariablesPanel from './react-pilot/VariablesPanelSwitch.svelte'; // VariablesPanel.svelte of web-panels, or its React copy with /?react=variables (E7.3, S5)
  import ActivityPanel from '@meldframe/web-activity/ActivityPanel.svelte';
  import { tabChange, toastFor, type RightTab } from '@meldframe/web-activity/tab';
  import { layerRows } from '@meldframe/web-document/layers';
  import { presetAction } from '@meldframe/web-shell/presets';
  import type { ExportScale } from '@meldframe/web-document/export';
  // The two switch files draw ExportDialog.svelte and ExportSection.svelte of web-export, or their React copies with /?react=export (E7.3, S2).
  import ExportDialog from './react-pilot/ExportDialogSwitch.svelte';
  import ExportSection from './react-pilot/ExportSectionSwitch.svelte';
  import { type Frame, pngPlan, savePng } from '@meldframe/web-export/png';
  import { saveBlob } from '@meldframe/web-export/save';
  import { dialogFrame, reviewSteps } from '@meldframe/web-export/ui';
  import { ringsOnCanvas, type Touched } from '@meldframe/web-document/rings';
  import { serveCapture, waitForSeq, type Req } from '@meldframe/web-document/capture-serve';
  import { bannerFor, connectionLabel } from '@meldframe/web-feedback/connection';
  import { reactFeedback as reactFeedbackOn } from './react-pilot/feedback';
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
    /** "Auto layout on canvas": whether the canvas draws laid-out boxes, and the switch (the palette entry). */
    layout?: LayoutSwitch;
    /** Capture requests of agents: where they come from, and how to tell the server this editor cannot serve one. */
    captures?: { subscribe: (fn: (r: Req) => void) => () => void; fail: (requestId: string, code: string, message: string) => void };
  }
  let { engine, file, actor, attribution = {}, layout, captures }: Props = $props();
  const reactFeedback = reactFeedbackOn(location.search); // the React copy of web-feedback (E7.3, S3): /?react=feedback; removed with the last Svelte page
  let autoLayout = $state(untrack(() => layout?.on ?? false));

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
  // The line of the slot in a flow that a dragged node would land in (a drag of a child of an auto layout frame).
  const indicator = $derived(preview?.kind === 'slot' ? preview.line : null);
  // The one overlay of the layout that can be open: the shortcut sheet or the command palette.
  let overlay = $state<'help' | 'palette' | null>(null);

  const dispatch = (a: Action) => void engine.dispatch(a);

  // The layers: what the server stamped when the file loaded, then what the app saw change since.
  const actors = $derived({ ...Object.fromEntries(Object.entries(attribution).map(([id, a]) => [id, a.actor])), ...snap.state.lastActor });
  const rows = $derived(layerRows(nodes, scene, snap.state.panels.collapsed, snap.state.selection, actors));

  // The touch rings of other actors are the app layer's (`state.activity.touches`); the canvas draws them on its own clock.
  const noRings: Touched[] = [];
  const touches = $derived.by(() => {
    if (snap.state.activity.touches.length === 0) return noRings;
    const c = pixiColors(theme);
    return ringsOnCanvas(snap.state.activity.touches, { epochMs: Date.now(), perfMs: performance.now() }, { person: c.person, agents: [c['agent-1'], c['agent-2'], c['agent-3']] });
  });

  // An agent asks for a PNG of a node: the canvas draws it offscreen and the page uploads it (see capture.ts).
  onMount(() =>
    captures?.subscribe((r) =>
      void serveCapture(r, {
        box: (id) => canvas?.captureBox(id) ?? null,
        render: (box, scale) => canvas!.capture(box, scale),
        seq: () => engine.snapshot().state.confirmedSeq,
        whenSeq: (min, timeoutMs) => waitForSeq(engine, min, timeoutMs),
        upload: async (id, headers, body) => (await fetch(`/captures/${encodeURIComponent(id)}`, { method: 'POST', headers, body })).status,
        fail: captures.fail,
      }),
    ),
  );

  // The frame an export acts on is the app layer's (`state.export.target`); the dialog keeps its own when the selection moves on. PNG (E4.3):
  // the canvas draws the frame offscreen as a capture is drawn, the plan follows the document (it reads the scene).
  const target = $derived(snap.state.export.target);
  let pngScale = $state<ExportScale>(1);
  const planOf = (f: Frame | null) => (scene && f ? pngPlan(canvas?.captureBox(f.id) ?? null, f, pngScale) : null);
  const dialogOf = $derived(dialogFrame(snap.state.export.dialog));
  const exportPng = (f: Frame | null) => f && void savePng({ box: (id) => canvas?.captureBox(id) ?? null, draw: (b, s) => canvas!.capture(b, s), save: saveBlob, notify: showNotice }, f, pngScale);

  // A refusal is shown for a few seconds, then goes away; a later snapshot does not cancel that (the timer is not tied to the effect).
  let toast = $state<ToastView | null>(null);
  const toaster = new Toaster((t) => (toast = t), duration.notice);
  const showNotice = (text: string) => toaster.show(text);
  let rightTab = $state<RightTab>('design');
  const goTab = (t: RightTab) => {
    const c = tabChange(rightTab, t);
    rightTab = c.tab;
    c.actions.forEach(dispatch);
  };
  // Only the events are tracked: a change of tab or a panel toggle must not show an old refusal again.
  $effect(() => {
    const events = snap.events;
    const text = untrack(() => toastFor(events, rightTab === 'activity' && snap.state.panels.properties));
    if (text) untrack(() => showNotice(text));
  });
  onDestroy(() => toaster.destroy());

  // The text being edited: the node and the text it had when the field opened (an unchanged commit makes no step).
  let editing = $state<{ id: string; before: string } | null>(null);
  const editedNode = $derived(editing ? textTarget(editing.id, nodes) : null);
  // The box comes from the document (not from what the canvas drew last), so it follows a change of the node at once: the
  // box the node is drawn at (with the layout on the canvas, the laid-out one; else the stored one).
  const editBox = $derived.by(() => {
    const n = editedNode;
    if (!n) return null;
    const o = worldOrigin(nodes, n.parentId, scene);
    const b = drawnBox(scene, n.id) ?? n;
    // The weight and the alignment are those the canvas draws the text with (the scene's, resolved).
    return textBoxStyle(camera, { x: o.x + b.x, y: o.y + b.y, w: b.w, h: b.h }, n.fontSize, scene.nodes.find((x) => x.id === n.id));
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
  // The banners over the canvas: the canvas could not start, and the connection while it is not live.
  const connection = $derived(bannerFor(snap.state.connection, snap.state.pending.length));
  const banners = $derived<Banner[]>([
    ...(canvasError ? [{ id: 'canvas-error', tone: 'danger' as const, text: `The canvas could not start: ${canvasError}` }] : []),
    ...(connection ? [connection] : []),
  ]);
  const onBanner = (_banner: string, action: string) => {
    if (action === 'reload') location.reload();
    else if (action === 'retry') dispatch({ type: 'reconnect', nowMs: Date.now() });
  };
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
  // The fit is redone once when the page's fonts settle (the measurer is given again, the texts change size), if the person has not
  // zoomed or panned since: what it first fitted to may have been measured with the stub.
  let fitCamera: Camera | null = null;
  let fitting = false;
  let setsAtFit = 0;
  let refitted = false;
  const fitAll = () => {
    fitting = true;
    canvas?.fitAll();
    fitting = false;
  };
  $effect(() => {
    if (ready && !fitted && nodes.length) {
      fitted = true;
      setsAtFit = engine.stats.measurerSets;
      fitAll();
    }
  });
  $effect(() => {
    void scene;
    untrack(() => {
      if (shouldRefit({ fitted, refitted, setsAtFit, sets: engine.stats.measurerSets, fitCamera, camera })) {
        refitted = true;
        fitAll();
      }
    });
  });

  // A drag, a resize or a drawn box is going on: an arrow key then is not a command (the gesture is the person's hands' business).
  let gesture = false;

  // The pointer reports faster than the screen draws: the slot is asked of the app layer once per animation frame, on the
  // latest move; the end of the gesture is never held back.
  const slotQuery = perFrame<MoveIntent>(
    { request: (cb) => requestAnimationFrame(cb), cancel: (id) => cancelAnimationFrame(id) },
    (i) => (preview = movePreviewFor(i, (ids, at, by) => engine.dropPreview(ids, at, by))),
  );
  onDestroy(() => slotQuery.cancel());

  /** The app layer says where the dragged flow children would land (a line) and which nodes follow the pointer. */
  function onMove(i: MoveIntent) {
    gesture = gestureGoing(i);
    if (!i.final && !i.cancelled) return slotQuery.push(i);
    slotQuery.cancel();
    preview = null;
    for (const a of moveActionsFor(i)) dispatch(a);
  }

  /** One node is resized at a time: the handles of a selection of several nodes do nothing yet. */
  function onResize(i: ResizeIntent) {
    gesture = gestureGoing(i);
    // The box is the one the node is drawn at (its origin stays when the layout places the node), and a node that is gone meanwhile (a change of someone else) ends the gesture quietly.
    preview = resizePreviewFor(i, nodes, scene, originLocked(snap.state.properties, i.ids[0] ?? ''));
    const a = resizeActionFor(i, nodes, scene);
    if (a) dispatch(a);
  }

  /** The canvas draws the box while it is dragged; the node is made when the gesture ends. */
  function onCreate(i: CreateIntent) {
    gesture = gestureGoing(i);
    const a = createActionFor(i, nodes, scene);
    if (a) dispatch(a);
  }

  /** Keys of the editor (tools, delete, undo, redo), unless the person is typing in a field. */
  function onKey(e: KeyboardEvent) {
    if (e.repeat || e.isComposing) return; // a held key must not undo a step per repeat; a composition is not a command
    if (ownsKeys(e.target as HTMLElement | null, e)) return;
    const key = layoutKey(e);
    if (key) {
      e.preventDefault();
      if (key === 'palette' && paletteBlocked(readOverlays(document), snap.state.export.dialog !== null)) return;
      if (key === 'help' || key === 'palette') overlay = overlay === key ? null : key;
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
    if (a?.type === 'arrowKey') {
      // The key is the editor's only when it changed the document (the layout is on, and the selection is a child of a flow that can
      // move that way): dispatch first, then take it from the browser; any other time it goes on to whatever else answers an arrow key.
      if (gesture) return;
      const before = engine.snapshot();
      engine.dispatch(a);
      if (documentChanged(before, engine.snapshot())) e.preventDefault();
      return;
    }
    if (a) { e.preventDefault(); dispatch(a); }
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
  canExport={target !== null} onExport={() => dispatch({ type: 'openExport' })}
  canVariant={target !== null} onVariant={() => target && dispatch({ type: 'makeVariant', id: target.id })}
  canDelete={deleteAction(snap.state.selection) !== null} onDelete={() => { const a = deleteAction(snap.state.selection); if (a) dispatch(a); }}
  zoomText={formatZoom(camera.zoom)}
  onZoom={(c) => (c === 'fit' ? canvas?.fitAll() : c === 'fit-selection' ? canvas?.fitSelection() : canvas && (camera = zoomCamera(c, camera, canvas.centre())))}
  connection={{ state: snap.state.connection.type, text: connectionLabel(snap.state.connection) }}
  helpOpen={overlay === 'help'} onHelp={(open) => (overlay = open ? 'help' : null)}
  paletteOpen={overlay === 'palette'} onPalette={(open) => (overlay = open ? 'palette' : null)}
  {autoLayout} onAutoLayout={layout ? () => (autoLayout = toggleWithNotice(layout, showNotice)) : undefined}
  tab={rightTab} activityBadge={snap.state.activity.unreviewedCount}
  onTab={goTab}
  {...shellParts}
>
  {#snippet canvasLayer()}
    <Canvas
      bind:this={canvas}
      scene={shown}
      selection={snap.state.selection}
      tool={toolMode(snap.state.tool)}
      bind:camera
      onCameraChange={(c) => { if (fitting) fitCamera = { ...c }; }}
      {theme}
      {touches}
      {indicator}
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
    {#if reactFeedback}
      {#await import('./react-pilot/FeedbackLayerHost.svelte') then host}
        <host.default {nodes} {ready} canAdd={snap.state.connection.type !== 'halted'} onAdd={() => canvas && dispatch(presetAction('desktop', canvas.centre()))} {toast} onCloseToast={() => toaster.dismiss()} {banners} onBanner={onBanner} />
      {:catch}
        <!-- the React chunk did not load: the Svelte copy is in the entry, so the banners, the toast and the empty state still draw -->
        <NoFrames {nodes} {ready} canAdd={snap.state.connection.type !== 'halted'} onAdd={() => canvas && dispatch(presetAction('desktop', canvas.centre()))} />
        <Toast {toast} onClose={() => toaster.dismiss()} />
        <BannerStack {banners} onAction={onBanner} />
      {/await}
    {:else}
      <NoFrames {nodes} {ready} canAdd={snap.state.connection.type !== 'halted'} onAdd={() => canvas && dispatch(presetAction('desktop', canvas.centre()))} />
      <Toast {toast} onClose={() => toaster.dismiss()} />
      <BannerStack {banners} onAction={onBanner} />
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
      <VariablesPanel panel={snap.state.variables} io={snap.state.variablesIo} target={snap.state.selection.length === 1 ? (snap.state.selection[0] ?? null) : null} {dispatch} events={snap.events} />
    </Section>
    <PropertiesPanel properties={snap.state.properties} picker={snap.state.picker ?? null} assets={snap.state.assets} selected={snap.state.selection.length} {dispatch} {...bindParts} />
    <ExportSection {target} scale={pngScale} onScale={(s) => (pngScale = s)} plan={planOf(target)} onPng={() => exportPng(target)} onHtml={() => dispatch({ type: 'openExport' })} />
  {/snippet}
  {#snippet activityPanel()}<ActivityPanel activity={snap.state.activity} checkpoints={snap.state.checkpoints} canCreate={snap.state.canCreateCheckpoint} status={snap.state.checkpointsStatus} self={actor} {dispatch} events={snap.events} />{/snippet}
</Layout>
<ExportDialog dialog={snap.state.export.dialog} unreviewed={snap.state.activity.unreviewedCount} scale={pngScale} onScale={(s) => (pngScale = s)} plan={planOf(dialogOf)} onPng={() => exportPng(dialogOf)} {dispatch} onReview={() => { reviewSteps().forEach(dispatch); goTab('activity'); }} />
