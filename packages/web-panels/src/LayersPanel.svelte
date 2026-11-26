<!--
  The layers panel: the tree of the file, the selection, and who changed each node last. It renders the rows it is
  given (`state.layers`: every text in them is made safe to show by the app layer) and reports clicks; it holds no
  state of its own and derives nothing from a name or an actor.
-->
<script lang="ts">
  import Icon from '@meldframe/web-chrome/Icon.svelte';
  import type { LayerRow } from '@meldframe/protocol';
  import { KIND_ICON as icon, changedBy, nameOf } from './layers';

  interface Props {
    rows: readonly LayerRow[];
    /** The actor of this page: its own changes carry no label. */
    self: string;
    onSelect: (id: string, additive: boolean) => void;
    onToggle: (id: string) => void;
  }
  let { rows, self, onSelect, onToggle }: Props = $props();
</script>

<div class="flex flex-col px-3 pb-3 text-body text-text-secondary" data-testid="layers">
  <h3 id="layers-title" class="m-0 py-2 text-body font-semibold text-text-strong">Layers</h3>
  <ul class="m-0 flex list-none flex-col gap-px p-0" aria-labelledby="layers-title">
    {#each rows as r (r.id)}
      {@const who = changedBy(r, self)}
      <li
        class="group flex h-(--mf-size-control-md) items-center gap-2 rounded-sm pr-2 {r.selected ? 'bg-selection-bg text-selection-text font-medium' : 'hov:bg-hairline'}"
        style:padding-left="calc(var(--mf-space-2) + {r.depth} * var(--mf-space-5))"
        data-testid="layer"
        data-id={r.id}
        data-selected={r.selected}
      >
        {#if r.hasChildren}
          <button type="button" class="flex h-full min-w-(--mf-size-control-sm) shrink-0 items-center justify-center rounded-xs text-text-muted" aria-label="{r.collapsed ? 'Expand' : 'Collapse'} {nameOf(r)}" aria-expanded={!r.collapsed} onclick={() => onToggle(r.id)}><Icon name={r.collapsed ? 'chevron-right' : 'chevron-down'} /></button>
        {:else}
          <span class="min-w-(--mf-size-control-sm) shrink-0"></span>
        {/if}
        <button type="button" class="flex h-full min-h-(--mf-size-control-sm) min-w-0 flex-1 items-center gap-2 truncate text-left" aria-current={r.selected ? 'true' : undefined} onclick={(e) => onSelect(r.id, e.shiftKey)}>
          <Icon name={icon[r.kind]} /><span class="truncate">{nameOf(r)}</span>
        </button>
        {#if who}
          <span class="flex min-w-0 max-w-24 shrink-0 items-center gap-1" title="Last changed by {who.name}" data-testid="actor" data-tone={who.tone}>
            <span class="truncate text-caption {r.selected ? 'text-selection-text' : 'text-text-muted'}">{who.name}</span>
            <span class="size-2 shrink-0 rounded-pill {who.dot}"></span>
          </span>
        {/if}
      </li>
    {/each}
    {#if rows.length === 0}
      <li class="py-2 text-text-muted">Nothing here yet.</li>
    {/if}
  </ul>
</div>
