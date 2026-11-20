<!--
  The layout with made-up content, at /?layout: for the layout check (layout-e2e.mjs) and for looking at the chrome without a
  server. The state is local to the page; nothing is read from or written to a file.
-->
<script lang="ts">
  import Layout from './Layout.svelte';
  import { HOME, formatZoom, type Camera } from '@meldframe/web-canvas/camera';
  import type { FramePreset } from '@meldframe/protocol';
  import { ownsKeys } from '@meldframe/web-document/edit';
  import { layoutKey, paletteBlocked, readOverlays } from './keys';
  import { presetSize } from './presets';
  import { zoomCamera } from './zoom';

  let tool = $state<'select' | 'frame' | 'rectangle' | 'text'>('select');
  let layersOpen = $state(true);
  let inspectorOpen = $state(true);
  let helpOpen = $state(false);
  let paletteOpen = $state(false);
  let tab = $state<'design' | 'activity'>('design');
  // The zoom menu changes a camera of its own here; fitting has no content to fit, so it goes home.
  let camera = $state<Camera>({ ...HOME });
  const toggle = (k: 'layers' | 'inspector') => (k === 'layers' ? (layersOpen = !layersOpen) : (inspectorOpen = !inspectorOpen));
  // The frames the New frame menu and the palette make here: boxes of the preset's size, for the browser check.
  let frames = $state<{ preset: FramePreset; w: number; h: number }[]>([]);
  // The same rules as the editor page: a key typed in a field is the field's, and the palette does not open over a menu or a dialog.
  function onKey(e: KeyboardEvent) {
    if (ownsKeys(e.target as HTMLElement | null, e)) return;
    const k = layoutKey(e);
    if (!k) return;
    e.preventDefault();
    if (k === 'palette' && paletteBlocked(readOverlays(document), false)) return;
    if (k === 'help') helpOpen = !helpOpen;
    else if (k === 'palette') paletteOpen = !paletteOpen;
    else toggle(k);
  }
  const rows = ['Landing Desktop', 'Hero', 'Hero Title', 'Pricing', 'Security'];
</script>

<svelte:window onkeydown={onKey} />

<Layout
  fileName="file_dev"
  {tool}
  onTool={(t) => (tool = t)}
  {layersOpen}
  {inspectorOpen}
  onToggleLayers={() => toggle('layers')}
  onToggleInspector={() => toggle('inspector')}
  canUndo={true}
  canRedo={false}
  onUndo={() => {}}
  onRedo={() => {}}
  onPreset={(p) => (frames = [...frames, { preset: p, ...presetSize(p) }])}
  canExport={true}
  onExport={() => {}}
  canVariant={true}
  onVariant={() => {}}
  zoomText={formatZoom(camera.zoom)}
  onZoom={(c) => (camera = c === 'fit' || c === 'fit-selection' ? { ...HOME } : zoomCamera(c, camera, { x: 0, y: 0 }))}
  connection={{ state: 'live', text: 'Connected' }}
  {helpOpen}
  onHelp={(o) => (helpOpen = o)}
  {paletteOpen}
  onPalette={(o) => (paletteOpen = o)}
  canDelete={frames.length > 0}
  onDelete={() => (frames = frames.slice(0, -1))}
  {tab}
  onTab={(t) => (tab = t)}
>
  {#snippet canvasLayer()}<div class="absolute inset-0 overflow-hidden bg-canvas">{#each frames as f, i (i)}<div data-testid="demo-frame" data-preset={f.preset} style:width="{f.w}px" style:height="{f.h}px" class="absolute top-0 left-0 rounded-sm border border-border bg-surface"></div>{/each}</div>{/snippet}
  {#snippet layersPanel()}<ul class="m-0 list-none p-2">{#each rows as r (r)}<li class="rounded-sm px-2 py-1">{r}</li>{/each}</ul>{/snippet}
  {#snippet designPanel()}<p class="m-0 p-4 text-text-muted">Design panel content</p><input aria-label="Name" data-testid="demo-field" class="m-4 h-(--mf-size-control-lg) rounded-md border border-border bg-field px-2 text-text" />{/snippet}
  {#snippet activityPanel()}<p class="m-0 p-4 text-text-muted">Activity panel content</p>{/snippet}
</Layout>
