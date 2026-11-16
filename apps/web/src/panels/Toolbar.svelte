<script lang="ts">
  import type { Action, AppState, Tool } from '@meldframe/protocol';

  let { state, dispatch, actor }: { state: AppState; dispatch: (a: Action) => unknown; actor: string } = $props();
  const tools: Tool[] = ['select', 'frame', 'rectangle', 'text'];
  const connection = $derived(
    state.connection.type === 'reconnecting' ? `reconnecting (${state.connection.attempt})` : state.connection.type,
  );
</script>

<header>
  {#each tools as tool}
    <button data-testid="tool-{tool}" class:active={state.tool === tool} onclick={() => dispatch({ type: 'setTool', tool })}>
      {tool}
    </button>
  {/each}
  <span class="gap"></span>
  <button data-testid="undo" disabled={!state.history.canUndo} onclick={() => dispatch({ type: 'undo' })}>undo</button>
  <button data-testid="redo" disabled={!state.history.canRedo} onclick={() => dispatch({ type: 'redo' })}>redo</button>
  <span class="gap"></span>
  <span class="muted" data-testid="actor">{actor}</span>
  <span class="muted" data-testid="connection">{connection}</span>
  <span class="muted" data-testid="pending">{state.pending.length} pending</span>
</header>

<style>
  header {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    height: var(--size-toolbar-height);
    padding: 0 var(--space-3);
    background: var(--colour-panel);
    border-bottom: var(--size-border) solid var(--colour-border);
  }
  button {
    background: transparent;
    color: var(--colour-text);
    border: var(--size-border) solid var(--colour-border);
    border-radius: var(--size-radius);
    padding: var(--space-1) var(--space-2);
    cursor: pointer;
  }
  button.active {
    border-color: var(--colour-accent);
    color: var(--colour-accent);
  }
  button:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .gap {
    flex: 1;
  }
  .muted {
    color: var(--colour-text-muted);
    font-size: var(--font-size-sm);
  }
</style>
