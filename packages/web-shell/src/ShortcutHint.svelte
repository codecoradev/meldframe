<!--
  The shortcut sheet: a card with every key and gesture the editor answers to today, in groups (the list is
  `shortcuts.ts`, checked against the real handlers), opened by `?` or the help button and closed by Escape. It scrolls when
  the window is short (the card around it sets the room it has). The command palette comes later (D10); this one only lists.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import IconButton from '@meldframe/web-chrome/IconButton.svelte';
  import { takeFocusBack } from '@meldframe/web-chrome/focus';
  import { onMac } from './platform';
  import { sheetGroups } from './shortcuts';

  let { onClose, autoLayout = true }: { onClose: () => void; autoLayout?: boolean } = $props();
  let root: HTMLElement | undefined = $state();
  // Focus goes in when the sheet opens and back to where it was when it closes; Escape acts only while the focus is in
  // the sheet, so an Escape meant for a menu or a text edit is not taken by it.
  onMount(() => {
    // Opened from the palette, the focus goes back to where the palette was opened from, which it remembered for this.
    const before = (takeFocusBack() as HTMLElement | null) ?? (document.activeElement as HTMLElement | null);
    root?.querySelector<HTMLElement>('button')?.focus();
    return () => { if (before && before.isConnected) before.focus(); };
  });
  const groups = $derived(sheetGroups(onMac(), autoLayout));
</script>

<div bind:this={root} role="dialog" tabindex="-1" aria-label="Keyboard shortcuts" onkeydown={(e) => { if (e.key === 'Escape') { e.stopPropagation(); onClose(); } }} data-testid="shortcut-hint" class="pointer-events-auto flex max-h-full w-(--mf-size-inspector) flex-col gap-1 overflow-y-auto rounded-lg border border-menu-border bg-surface-raised p-3 text-small text-text shadow-popover">
  <div class="flex items-center justify-between font-semibold text-text-strong">Shortcuts<IconButton label="Close shortcuts" icon="close" size={24} onclick={onClose} /></div>
  {#each groups as g (g.id)}
    <section aria-labelledby="shortcut-group-{g.id}" class="flex flex-col gap-1 pt-2">
      <h2 id="shortcut-group-{g.id}" class="text-caption font-semibold text-text-muted">{g.title}</h2>
      {#each g.rows as row (row.id)}
        <div class="flex items-center justify-between gap-3"><span>{row.name}</span><span class="flex shrink-0 gap-1">{#each row.keys as key (key)}<kbd class="rounded-xs border border-menu-border bg-field px-1 font-mono text-caption">{key}</kbd>{/each}</span></div>
      {/each}
    </section>
  {/each}
  <p class="pt-2 text-caption text-text-muted">The view keys work while the canvas has the focus.</p>
</div>
