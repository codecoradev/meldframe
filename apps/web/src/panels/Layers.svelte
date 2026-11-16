<script lang="ts">
  import type { Action, AppState, Node } from '@meldframe/protocol';
  import { layerRows } from '../layers';

  let { nodes, state, actor, dispatch }: { nodes: Node[]; state: AppState; actor: string; dispatch: (a: Action) => unknown } = $props();
  const rows = $derived(layerRows(nodes, state.panels.collapsed, state.lastActor));
  const click = (e: MouseEvent, id: string) =>
    dispatch(e.shiftKey ? { type: 'toggleSelect', id } : { type: 'select', ids: [id] });
</script>

<aside data-testid="layers">
  <h2>Layers</h2>
  {#each rows as row (row.id)}
    <div
      class="row"
      class:selected={state.selection.includes(row.id)}
      data-testid="layer-{row.id}"
      style:padding-left="calc(var(--space-2) + {row.depth} * var(--size-indent))"
    >
      {#if row.hasChildren}
        <button class="twist" aria-label="toggle children" onclick={() => dispatch({ type: 'toggleCollapsed', id: row.id })}>
          {row.collapsed ? '+' : '-'}
        </button>
      {:else}
        <span class="twist"></span>
      {/if}
      <button class="name" onclick={(e) => click(e, row.id)}>
        <span class="kind">{row.kind}</span> {row.name || row.id}
      </button>
      {#if row.actor}
        <span class="actor" class:other={row.actor !== actor} data-testid="actor-{row.id}">{row.actor}</span>
      {/if}
    </div>
  {/each}
</aside>

<style>
  aside {
    width: var(--size-panel-width);
    background: var(--colour-panel);
    border-right: var(--size-border) solid var(--colour-border);
    overflow-y: auto;
  }
  h2 {
    margin: 0;
    padding: var(--space-2);
    font-size: var(--font-size-sm);
    color: var(--colour-text-muted);
    font-weight: normal;
  }
  .row {
    display: flex;
    align-items: center;
    height: var(--size-row-height);
    gap: var(--space-1);
  }
  .row.selected {
    background: var(--colour-border);
  }
  button {
    background: transparent;
    color: inherit;
    border: 0;
    padding: 0;
    cursor: pointer;
    font: inherit;
  }
  .twist {
    width: var(--size-indent);
    text-align: center;
  }
  .name {
    flex: 1;
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .kind {
    color: var(--colour-text-muted);
    font-size: var(--font-size-sm);
  }
  .actor {
    margin-right: var(--space-2);
    font-size: var(--font-size-sm);
    color: var(--colour-text-muted);
  }
  .actor.other {
    color: var(--colour-agent);
  }
</style>
