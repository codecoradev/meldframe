<script lang="ts">
  import type { Engine } from './engine';
  import Canvas from './canvas/Canvas.svelte';
  import Layers from './panels/Layers.svelte';
  import Properties from './panels/Properties.svelte';
  import Toolbar from './panels/Toolbar.svelte';

  let { engine, actor }: { engine: Engine; actor: string } = $props();
  // svelte-ignore state_referenced_locally
  let snap = $state.raw(engine.snapshot());
  $effect(() => engine.subscribe((s) => (snap = s)));
  const dispatch = (a: Parameters<Engine['dispatch']>[0]) => engine.dispatch(a);
</script>

<div class="shell">
  {#if snap.state.connection.type === 'halted'}
    <div class="halted" role="alert" data-testid="halted">
      The file and the server no longer fit. Reload to start again from the server's copy.
      <button onclick={() => location.reload()}>Reload</button>
    </div>
  {/if}
  <Toolbar state={snap.state} {dispatch} {actor} />
  <main>
    {#if snap.state.panels.layers}
      <Layers nodes={snap.nodes} state={snap.state} {actor} {dispatch} />
    {/if}
    <Canvas nodes={snap.nodes} selection={snap.state.selection} tool={snap.state.tool} {dispatch} />
    {#if snap.state.panels.properties}
      <Properties properties={snap.state.properties} {dispatch} />
    {/if}
  </main>
</div>

<style>
  :global(html, body) {
    margin: 0;
    height: 100%;
    background: var(--colour-bg);
    color: var(--colour-text);
    font-family: var(--font-ui);
    font-size: var(--font-size-md);
  }
  :global(#app), .shell {
    height: 100%;
  }
  .shell {
    display: flex;
    flex-direction: column;
  }
  .halted {
    padding: var(--space-2) var(--space-3);
    background: var(--colour-danger);
    color: var(--colour-bg);
  }
  main {
    flex: 1;
    display: flex;
    min-height: 0;
  }
</style>
