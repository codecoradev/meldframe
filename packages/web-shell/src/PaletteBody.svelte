<!--
  The inside of the command palette: a search field and the list it narrows (`palette.ts` decides what is listed, in what
  order, and what the arrows and Enter do). A combobox in the pattern of the ARIA guide: the focus stays in the field, the
  highlighted row is named by `aria-activedescendant`, a row that cannot run now is listed but disabled. It keeps view state
  only (the text typed and the row the pointer chose); the dialog around it is `Palette.svelte`.
-->
<script lang="ts">
  import { layoutKey } from './keys';
  import { filterEntries, highlightAt, paletteKey, resultsText, type PaletteEntry } from './palette';

  interface Props {
    entries: PaletteEntry[];
    /** The person ran an entry (Enter, or a click on a row that can run). */
    onRun: (e: PaletteEntry) => void;
    /** Ctrl+K again: the browser's own use of it (a search bar) is held back and the palette closes. */
    onClose: () => void;
  }
  let { entries, onRun, onClose }: Props = $props();

  let query = $state('');
  // The row the pointer chose; typing forgets it, and the highlight starts again on the first row that can run.
  let picked = $state<string | null>(null);
  const shown = $derived(filterEntries(entries, query));
  const at = $derived(highlightAt(shown, picked));
  const rowId = (e: PaletteEntry) => `palette-option-${e.id}`;
  const keydown = (ev: KeyboardEvent) => {
    if (ev.isComposing) return;
    if (layoutKey(ev) === 'palette') {
      ev.preventDefault();
      ev.stopPropagation();
      onClose();
      return;
    }
    const k = paletteKey(ev.key, at, shown);
    if (!k) return;
    ev.preventDefault();
    if (k.kind === 'run') onRun(shown[k.index]!);
    else picked = shown[k.index]!.id;
  };
  // The highlighted row stays in view as the arrows move it through a long list.
  $effect(() => {
    const e = shown[at];
    if (e) document.getElementById(rowId(e))?.scrollIntoView?.({ block: 'nearest' });
  });
</script>

<div class="flex flex-col">
  <input
    bind:value={query}
    oninput={() => (picked = null)}
    onkeydown={keydown}
    type="text"
    role="combobox"
    aria-expanded="true"
    aria-controls="palette-list"
    aria-autocomplete="list"
    aria-activedescendant={shown[at] ? rowId(shown[at]) : undefined}
    aria-label="Search commands"
    placeholder="Search commands…"
    autocomplete="off"
    spellcheck="false"
    data-testid="palette-input"
    class="h-(--mf-size-control-wide) w-full border-b border-hairline bg-transparent px-4 text-body text-text-strong outline-none placeholder:text-text-muted"
  />
  <div id="palette-list" role="listbox" aria-label="Commands" data-testid="palette-list" class="max-h-80 overflow-y-auto p-1">
    {#each shown as e, i (e.id)}
      <!-- The keys of the list are the field's (see keydown); a row is for the pointer. A press must not take the focus from the field. -->
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <div
        id={rowId(e)}
        role="option"
        tabindex="-1"
        aria-selected={i === at}
        aria-disabled={!e.enabled}
        data-testid="palette-option"
        data-id={e.id}
        onmousedown={(ev) => ev.preventDefault()}
        onpointermove={() => { if (e.enabled && i !== at) picked = e.id; }}
        onclick={() => e.enabled && onRun(e)}
        class="flex h-(--mf-size-control-lg) items-center justify-between gap-4 rounded-sm px-3 text-small text-text {i === at ? 'bg-control' : ''} {e.enabled ? 'cursor-default' : 'opacity-(--mf-opacity-disabled)'}"
      >
        <span class="truncate">{e.label}</span>
        <span class="flex shrink-0 items-center gap-2 text-caption text-text-muted">
          <span>{e.group}</span>
          {#if e.detail}<span class="font-mono">{e.detail}</span>{/if}
          {#if e.shortcut}<kbd class="rounded-xs border border-menu-border bg-field px-1 font-mono">{e.shortcut}</kbd>{/if}
        </span>
      </div>
    {/each}
  </div>
  {#if shown.length === 0}<p class="m-0 px-4 py-3 text-small text-text-muted" data-testid="palette-empty">Nothing matches.</p>{/if}
  <p class="sr-only" role="status">{resultsText(shown.length)}</p>
</div>
