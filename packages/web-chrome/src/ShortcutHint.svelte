<!--
  The shortcut hint: a small card of the keys the editor answers to, opened by `?` or the help button and closed by
  Escape. The full sheet and the command palette come later (D10); this one only lists.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import IconButton from './IconButton.svelte';
  import { platformLabels } from './platform';

  let { onClose }: { onClose: () => void } = $props();
  let root: HTMLElement | undefined = $state();
  // Focus goes in when the hint opens and back to where it was when it closes; Escape acts only while the focus is in
  // the hint, so an Escape meant for a menu or a text edit is not taken by it.
  onMount(() => {
    const before = document.activeElement as HTMLElement | null;
    root?.querySelector<HTMLElement>('button')?.focus();
    return () => { if (before && before.isConnected) before.focus(); };
  });
  const keys = platformLabels();
  const rows: [string, string][] = [
    ['Select', 'V'], ['Frame', 'F'], ['Rectangle', 'R'], ['Text', 'T'],
    ['Layers panel', keys.layers], ['Right panel', keys.inspector],
    ['Undo', keys.undo], ['Redo', keys.redo], ['Delete', 'Delete'], ['Shortcuts', '?'],
  ];
</script>

<div bind:this={root} role="dialog" tabindex="-1" aria-label="Keyboard shortcuts" onkeydown={(e) => { if (e.key === 'Escape') { e.stopPropagation(); onClose(); } }} data-testid="shortcut-hint" class="flex w-(--mf-size-layers) flex-col gap-1 rounded-lg border border-menu-border bg-surface-raised p-3 text-small text-text shadow-popover">
  <div class="flex items-center justify-between font-semibold text-text-strong">Shortcuts<IconButton label="Close shortcuts" icon="close" size={24} onclick={onClose} /></div>
  {#each rows as [name, key] (name)}
    <div class="flex items-center justify-between"><span>{name}</span><kbd class="rounded-xs border border-menu-border bg-field px-1 font-mono text-caption">{key}</kbd></div>
  {/each}
</div>
