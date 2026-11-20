<!--
  The layout of the editor: the canvas is the window and the chrome floats over it, as the Main, Panels and A11y boards
  draw it. A Layers card on the left (260), ONE card on the right (288 on Design, 360 on Activity) with the tabs
  Design and Activity (the page chooses the tab), two 52 px cards that stand in for the panels when they are closed,
  the tool bar at the bottom centre, a status strip and a help button. It draws what it is given and reports clicks: no state of the document, no rules (the app layer has them).
  Share and the account are not part of it.
-->
<script lang="ts">
  import { tick, type Snippet } from 'svelte';
  import IconButton from './IconButton.svelte';
  import Menu from './Menu.svelte';
  import ShortcutHint from './ShortcutHint.svelte';
  import Tabs from './Tabs.svelte';
  import TooltipProvider from './TooltipProvider.svelte';
  import { onMac, platformLabels } from './platform';
  import { PANEL_INSET, fileItems, reserved, rightWidth, type RightTab } from './layout';
  import { zoomItems, type ZoomCommand } from './zoom';

  type Tool = 'select' | 'frame' | 'rectangle' | 'text';
  interface Props {
    fileName: string;
    tool: Tool;
    onTool: (t: Tool) => void;
    layersOpen: boolean;
    inspectorOpen: boolean;
    onToggleLayers: () => void;
    onToggleInspector: () => void;
    canUndo: boolean;
    canRedo: boolean;
    onUndo: () => void;
    onRedo: () => void;
    onPreset: (p: 'desktop' | 'tablet' | 'mobile') => void;
    /** The file menu: the Export dialog for the selected frame, and a variant of it. */
    canExport: boolean;
    onExport: () => void;
    canVariant: boolean;
    onVariant: () => void;
    /** The zoom as the status strip writes it (`100%`) and the command of a row of its menu; the page carries it out. */
    zoomText: string;
    onZoom: (c: ZoomCommand) => void;
    connection: { state: string; text: string };
    helpOpen: boolean;
    onHelp: (open: boolean) => void;
    /** The tab of the right card (Design or Activity) and the person's choice of another. */
    tab: RightTab;
    onTab: (t: RightTab) => void;
    /** The count of batches to review, from the app layer: shown on the Activity tab when above zero. */
    activityBadge?: number;
    canvasLayer: Snippet;
    layersPanel: Snippet;
    designPanel: Snippet;
    activityPanel: Snippet;
  }
  let { fileName, tool, onTool, layersOpen, inspectorOpen, onToggleLayers, onToggleInspector, canUndo, canRedo, onUndo, onRedo, onPreset, canExport, onExport, canVariant, onVariant, zoomText, onZoom, connection, helpOpen, onHelp, tab, onTab, activityBadge = 0, canvasLayer, layersPanel, designPanel, activityPanel }: Props = $props();

  const tools: { tool: Tool; label: string; key: string; icon: 'select' | 'frame' | 'rectangle' | 'text' }[] = [
    { tool: 'select', label: 'Select', key: 'V', icon: 'select' },
    { tool: 'frame', label: 'Frame', key: 'F', icon: 'frame' },
    { tool: 'rectangle', label: 'Rectangle', key: 'R', icon: 'rectangle' },
    { tool: 'text', label: 'Text', key: 'T', icon: 'text' },
  ];
  const fileMenu = $derived(fileItems({ canExport, canVariant, onExport, onVariant }));
  const presetItems = (['desktop', 'tablet', 'mobile'] as const).map((p) => ({
    id: p, label: `New ${p} frame`, onSelect: () => onPreset(p), testId: `preset-${p}`,
  }));
  const keys = platformLabels();
  const zoomRows = $derived(zoomItems({ mac: onMac(), onZoom }));
  // A card and its folded form swap one header button for another, so the focus would be lost: after a toggle made
  // from that button, the focus goes to the button of the same panel in its new state (marked data-focus).
  const toggleFrom = (panel: 'layers' | 'inspector', toggle: () => void) => () => {
    toggle();
    void tick().then(() => document.querySelector<HTMLElement>(`[data-owns-keys] [data-focus=${panel}]`)?.focus());
  };
  const flipLayers = $derived(toggleFrom('layers', onToggleLayers));
  const flipInspector = $derived(toggleFrom('inspector', onToggleInspector));
  const rightTabs = $derived<{ id: RightTab; label: string; badge?: number; badgeLabel?: string }[]>([{ id: 'design', label: 'Design' }, { id: 'activity', label: 'Activity', badge: activityBadge, badgeLabel: activityBadge === 1 ? 'batch to review' : 'batches to review' }]);
  const select = (id: string) => { const t = rightTabs.find((x) => x.id === id); if (t) onTab(t.id); };
  const edge = $derived(reserved({ layers: layersOpen, inspector: inspectorOpen, tab }));
  const width = $derived(rightWidth(tab));
  const card = 'absolute flex flex-col overflow-hidden rounded-lg border border-border bg-surface shadow-float';
  const topCard = 'absolute top-4 flex h-(--mf-size-top-card) items-center gap-2 rounded-lg border border-border bg-surface px-2 shadow-float';
  const divider = 'mx-1 h-5 w-px bg-divider';
</script>

<TooltipProvider>
<div class="fixed inset-0 overflow-hidden bg-canvas font-sans text-body text-text">
  <!-- The canvas keeps the area between the open cards, so a fit, a handle or a click is never under a card. -->
  <div class="absolute inset-y-0" style:left="{edge.left}px" style:right="{edge.right}px">{@render canvasLayer()}</div>
  <output class="sr-only" aria-live="off" data-testid="tool" data-tool={tool}>{tool}</output>

  {#if layersOpen}
    <section aria-label="Left panel" data-owns-keys data-testid="layers-card" class="{card} top-3 bottom-3 left-3 w-(--mf-size-layers)">
      <header class="flex h-(--mf-size-top-card) shrink-0 items-center gap-2 border-b border-hairline px-2">
        <Menu label="File menu" icon="menu" items={fileMenu} testId="file-menu" />
        <span class="min-w-0 flex-1 truncate font-semibold text-text-strong" title={fileName}>{fileName}</span>
        <IconButton label="Layers panel" icon="panel-left" pressed={true} onclick={flipLayers} data-focus="layers" />
      </header>
      <div class="min-h-0 flex-1 overflow-y-auto">{@render layersPanel()}</div>
    </section>
  {:else}
    <div class="{topCard} left-4" data-owns-keys data-testid="top-left-card">
      <Menu label="File menu" icon="menu" items={fileMenu} testId="file-menu" />
      <span class="max-w-48 truncate px-2 font-medium" title={fileName}>{fileName}</span>
      <IconButton label="Layers panel" icon="panel-left" pressed={false} onclick={flipLayers} data-focus="layers" />
    </div>
  {/if}

  {#if inspectorOpen}
    <section aria-label="Right panel" data-owns-keys data-testid="right-card" class="{card} top-3 right-3 bottom-3" style:width="{width}px">
      <header class="flex h-(--mf-size-top-card) shrink-0 items-center gap-2 border-b border-hairline px-2">
        <IconButton label="Right panel" icon="panel-right" pressed={true} onclick={flipInspector} data-focus="inspector" />
        <Tabs label="Panel sections" idPrefix="right" selected={tab} onSelect={select} tabs={rightTabs} />
      </header>
      <div id="right-panel-{tab}" role="tabpanel" aria-labelledby="right-tab-{tab}" class="min-h-0 flex-1 overflow-y-auto">{#if tab === 'activity'}{@render activityPanel()}{:else}{@render designPanel()}{/if}</div>
    </section>
  {:else}
    <div class="{topCard} right-4" data-owns-keys data-testid="top-right-card">
      <IconButton label="Right panel" icon="panel-right" pressed={false} onclick={flipInspector} data-focus="inspector" />
    </div>
  {/if}

  <div role="group" aria-label="Tools" data-testid="toolbar" class="absolute bottom-5 left-1/2 flex -translate-x-1/2 items-center gap-1 rounded-lg border border-border bg-surface p-2 shadow-float">
    {#each tools as t (t.tool)}
      <IconButton label={t.label} icon={t.icon} size={36} shortcut={t.key} pressed={tool === t.tool} onclick={() => onTool(t.tool)} data-testid="tool-{t.tool}" />
    {/each}
    <span class={divider} aria-hidden="true"></span>
    <Menu label="New frame" icon="plus" size={36} items={presetItems} side="top" testId="frame-presets" />
    <span class={divider} aria-hidden="true"></span>
    <IconButton label="Undo" icon="undo" size={36} shortcut={keys.undo} disabled={!canUndo} onclick={onUndo} data-testid="undo" />
    <IconButton label="Redo" icon="redo" size={36} shortcut={keys.redo} disabled={!canRedo} onclick={onRedo} data-testid="redo" />
    <span class={divider} aria-hidden="true"></span>
    <IconButton label="Layers panel" icon="panel-left" size={36} shortcut={keys.layers} pressed={layersOpen} onclick={onToggleLayers} data-testid="toggle-layers" />
    <IconButton label="Right panel" icon="panel-right" size={36} shortcut={keys.inspector} pressed={inspectorOpen} onclick={onToggleInspector} data-testid="toggle-properties" />
  </div>

  <div class="absolute bottom-5 left-4" style:left={layersOpen ? `calc(${PANEL_INSET}px + var(--mf-size-layers) + var(--mf-space-4))` : undefined}>
    <IconButton label="Shortcuts" icon="help" size={32} shortcut="?" pressed={helpOpen} onclick={() => onHelp(!helpOpen)} />
  </div>
  {#if helpOpen}
    <div class="pointer-events-none absolute top-4 bottom-20 flex items-end" style:left={layersOpen ? `calc(${PANEL_INSET}px + var(--mf-size-layers) + var(--mf-space-4))` : 'var(--mf-space-4)'}><ShortcutHint onClose={() => onHelp(false)} /></div>
  {/if}

  <div class="absolute bottom-5 flex flex-col items-end gap-1 text-caption text-text-muted" style:right={inspectorOpen ? `calc(${PANEL_INSET}px + ${width}px + var(--mf-space-4))` : 'var(--mf-space-5)'}>
    <output class="flex items-center gap-2" data-testid="connection" data-state={connection.state}><span class="size-2 rounded-pill {connection.state === 'live' ? 'bg-success' : 'bg-warning'}"></span>{connection.text}</output>
    <Menu label="Zoom {zoomText}" text={zoomText} items={zoomRows} side="top" align="end" testId="zoom" />
  </div>
</div>
</TooltipProvider>
