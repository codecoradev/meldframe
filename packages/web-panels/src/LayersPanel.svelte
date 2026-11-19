<!--
  The layers panel: the tree of the file, the selection, and who changed each node last. It renders the rows it is
  given and reports clicks; it holds no state of its own.
-->
<script lang="ts">
  import { actorLabel, type LayerRow } from '@meldframe/web-document/layers';

  interface Props {
    rows: LayerRow[];
    file: string;
    /** The actor of this page: its own changes carry no label. */
    self: string;
    onSelect: (id: string, additive: boolean) => void;
    onToggle: (id: string) => void;
  }
  let { rows, file, self, onSelect, onToggle }: Props = $props();
</script>

<aside class="flex w-60 shrink-0 flex-col border-r border-hairline bg-sidebar text-small text-text" aria-label="Layers" data-testid="layers">
  <header class="truncate border-b border-hairline px-3 py-2 text-text-muted" title={file}>{file}</header>
  <ul class="min-h-0 flex-1 overflow-y-auto py-1">
    {#each rows as r (r.id)}
      {@const who = r.actor && r.actor !== self ? actorLabel(r.actor) : null}
      <li
        class="flex items-center gap-1 border-l-2 pr-2 {r.selected ? 'border-accent bg-field text-text-strong' : 'border-transparent hover:bg-field'}"
        style:padding-left="{r.depth * 12 + 8}px"
        data-testid="layer"
        data-id={r.id}
        data-selected={r.selected}
      >
        {#if r.hasChildren}
          <button type="button" class="w-4 text-text-muted" aria-label={r.collapsed ? 'Expand' : 'Collapse'} aria-expanded={!r.collapsed} onclick={() => onToggle(r.id)}>{r.collapsed ? '▸' : '▾'}</button>
        {:else}
          <span class="w-4"></span>
        {/if}
        <button type="button" class="min-w-0 flex-1 truncate py-1 text-left" onclick={(e) => onSelect(r.id, e.shiftKey)}>
          <span class="text-text-muted">{r.kind}</span> {r.name}
        </button>
        {#if who}
          <span
            class="shrink-0 rounded-pill px-2 text-caption {who.kind === 'agent' ? 'bg-agent-1 text-on-agent' : 'bg-person text-on-accent'}"
            title="Last changed by {who.name}"
            data-testid="actor"
          >{who.name}</span>
        {/if}
      </li>
    {/each}
    {#if rows.length === 0}
      <li class="px-3 py-2 text-text-muted">Nothing here yet.</li>
    {/if}
  </ul>
</aside>
