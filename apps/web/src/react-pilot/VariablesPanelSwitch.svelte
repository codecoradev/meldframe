<!--
  The temporary switch of the Variables panel (docs/react-web-plan.md, section 7, slice S5): what Editor.svelte imports in
  place of VariablesPanel.svelte. Without `/?react=variables` it draws the Svelte panel; with it, the React panel in an island,
  loaded on demand so that the editor's bundle does not carry React. If the React chunk of the panel does not load, the Svelte panel is
  drawn instead. The three dialogs the panel's buttons open (the manage view, Import variables, Export variables) are not drawn here
  (nor by either panel): they are open by the app layer's state and Editor.svelte draws them beside the Export dialog, each through a switch of
  its own (the React dialog with `/?react=manage` (S9) or `/?react=io` (S10), the Svelte dialog without it), so that a dialog stays when this
  panel goes (the right card folds, or the tab changes). The host element draws nothing of its own (`display: contents`). It goes with the
  switch when the last slice removes the Svelte shell.
-->
<script lang="ts">
  import type { ComponentProps } from 'svelte';
  import VariablesPanel from '@meldframe/web-panels/VariablesPanel.svelte';
  import { reactFor } from './pilot';

  let props: ComponentProps<typeof VariablesPanel> = $props();
  // The React side, island helper included, loads on demand: the editor's bundle does not carry react-dom.
  const react = reactFor('variables', location.search)
    ? Promise.all([import('@meldframe/web-chrome/react/island'), import('@meldframe/web-panels/react/VariablesPanel')]).then(([i, m]) => ({ island: i.island, component: m.VariablesPanel }))
    : null;
</script>

{#if react}
  {#await react then r}
    <div class="contents" use:r.island={{ component: r.component, props: { panel: props.panel, target: props.target, dispatch: props.dispatch } }}></div>
  {:catch}
    <!-- the React chunk did not load: the Svelte panel is in the entry, so the variables are still there -->
    <VariablesPanel {...props} />
  {/await}
{:else}
  <VariablesPanel {...props} />
{/if}
