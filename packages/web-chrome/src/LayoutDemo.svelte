<!--
  The layout with made-up content, at /?layout: for the layout check (layout-e2e.mjs) and for looking at the chrome without a
  server. The state is local to the page; nothing is read from or written to a file.
-->
<script lang="ts">
  import Layout from './Layout.svelte';
  import { layoutKey } from './keys';

  let tool = $state<'select' | 'frame' | 'rectangle' | 'text'>('select');
  let layersOpen = $state(true);
  let inspectorOpen = $state(true);
  let helpOpen = $state(false);
  const toggle = (k: 'layers' | 'inspector') => (k === 'layers' ? (layersOpen = !layersOpen) : (inspectorOpen = !inspectorOpen));
  const rows = ['Landing Desktop', 'Hero', 'Hero Title', 'Pricing', 'Security'];
</script>

<svelte:window onkeydown={(e) => { const k = layoutKey(e); if (!k) return; e.preventDefault(); if (k === 'help') helpOpen = !helpOpen; else toggle(k); }} />

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
  onPreset={() => {}}
  canExport={true}
  onExport={() => {}}
  canVariant={true}
  onVariant={() => {}}
  zoomText="100%"
  connection={{ state: 'live', text: 'Connected' }}
  {helpOpen}
  onHelp={(o) => (helpOpen = o)}
>
  {#snippet canvasLayer()}<div class="absolute inset-0 bg-canvas"></div>{/snippet}
  {#snippet layersPanel()}<ul class="m-0 list-none p-2">{#each rows as r (r)}<li class="rounded-sm px-2 py-1">{r}</li>{/each}</ul>{/snippet}
  {#snippet designPanel()}<p class="m-0 p-4 text-text-muted">Design panel content</p>{/snippet}
</Layout>
