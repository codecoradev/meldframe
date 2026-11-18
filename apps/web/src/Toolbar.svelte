<!--
  The tool bar: the four tools, the presets, PNG export and a variant of the selected frame, undo and redo, and the two panels. It shows app
  state and dispatches actions only.
-->
<script lang="ts">
  import type { Action, AppState, FramePreset, Tool } from '@meldframe/protocol';
  import { EXPORT_SCALES, type ExportScale } from '@meldframe/web-document/export';

  interface Props {
    state: AppState;
    dispatch: (a: Action) => void;
    /** Makes a frame of a preset in the middle of the view. */
    onPreset: (p: FramePreset) => void;
    /** Whether the selection is one frame, which PNG export saves. */
    canExport: boolean;
    /** Saves the selected frame as a PNG at this scale. */
    onExport: (scale: ExportScale) => void;
    /** The id of the one selected frame, which a variant copies; none when the selection is anything else. */
    variantOf: string | null;
  }
  let { state, dispatch, onPreset, canExport, onExport, variantOf }: Props = $props();
  const presets: { preset: FramePreset; label: string }[] = [
    { preset: 'desktop', label: 'Desktop' },
    { preset: 'tablet', label: 'Tablet' },
    { preset: 'mobile', label: 'Mobile' },
  ];

  const tools: { tool: Tool; label: string; key: string }[] = [
    { tool: 'select', label: 'Select', key: 'V' },
    { tool: 'frame', label: 'Frame', key: 'F' },
    { tool: 'rectangle', label: 'Rectangle', key: 'R' },
    { tool: 'text', label: 'Text', key: 'T' },
  ];
  const base = 'rounded-md px-3 py-1.5 text-small disabled:text-text-faint';
</script>

<div role="toolbar" aria-label="Tools" data-testid="toolbar" class="absolute bottom-4 left-1/2 flex -translate-x-1/2 items-center gap-1 rounded-lg bg-surface-raised p-1 shadow-float">
  {#each tools as t (t.tool)}
    <button
      type="button"
      class="{base} {state.tool === t.tool ? 'bg-accent text-on-accent' : 'text-text hover:bg-field'}"
      aria-pressed={state.tool === t.tool}
      title="{t.label} ({t.key})"
      data-testid="tool-{t.tool}"
      onclick={() => dispatch({ type: 'setTool', tool: t.tool })}
    >{t.label}</button>
  {/each}
  <span class="mx-1 h-5 w-px bg-hairline" aria-hidden="true"></span>
  {#each presets as p (p.preset)}
    <button type="button" class="{base} text-text-muted hover:bg-field hover:text-text" title="New {p.label} frame" data-testid="preset-{p.preset}" onclick={() => onPreset(p.preset)}>{p.label}</button>
  {/each}
  <span class="mx-1 h-5 w-px bg-hairline" aria-hidden="true"></span>
  {#each EXPORT_SCALES as scale (scale)}
    <button type="button" class="{base} whitespace-nowrap text-text-muted hover:bg-field hover:text-text" disabled={!canExport} title="Save the selected frame as a PNG at {scale}x" data-testid="export-{scale}x" onclick={() => onExport(scale)}>PNG {scale}x</button>
  {/each}
  <button type="button" class="{base} whitespace-nowrap text-text-muted hover:bg-field hover:text-text" disabled={variantOf === null} title="Copy the selected frame beside it as a variant" data-testid="make-variant" onclick={() => variantOf !== null && dispatch({ type: 'makeVariant', id: variantOf })}>Variant</button>
  <span class="mx-1 h-5 w-px bg-hairline" aria-hidden="true"></span>
  <button type="button" class="{base} text-text hover:bg-field" disabled={!state.history.canUndo} title="Undo (Ctrl+Z)" data-testid="undo" onclick={() => dispatch({ type: 'undo' })}>Undo</button>
  <button type="button" class="{base} text-text hover:bg-field" disabled={!state.history.canRedo} title="Redo (Ctrl+Shift+Z)" data-testid="redo" onclick={() => dispatch({ type: 'redo' })}>Redo</button>
  <span class="mx-1 h-5 w-px bg-hairline" aria-hidden="true"></span>
  <button type="button" class="{base} {state.panels.layers ? 'text-text-strong' : 'text-text-muted'} hover:bg-field" aria-pressed={state.panels.layers} data-testid="toggle-layers" onclick={() => dispatch({ type: 'togglePanel', panel: 'layers' })}>Layers</button>
  <button type="button" class="{base} {state.panels.properties ? 'text-text-strong' : 'text-text-muted'} hover:bg-field" aria-pressed={state.panels.properties} data-testid="toggle-properties" onclick={() => dispatch({ type: 'togglePanel', panel: 'properties' })}>Properties</button>
</div>
