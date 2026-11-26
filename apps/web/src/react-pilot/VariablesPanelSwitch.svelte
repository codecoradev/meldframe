<!--
  The temporary switch of the Variables panel (docs/react-web-plan.md, section 7, slice S5): what Editor.svelte imports in
  place of VariablesPanel.svelte. Without `/?react=variables` it draws the Svelte panel; with it, the React panel in an island,
  loaded on demand so that the editor's bundle does not carry React. The three dialogs of the panel (the manage view, Import
  variables, Export variables) cannot be drawn inside a React root while they are Svelte, so this file draws them beside the island
  and owns whether the manage view and Import variables are open (the React panel only reports a press of the two buttons). The
  manage view is `ManageDialogSwitch.svelte`, which is the React dialog with `/?react=manage` (S9); Import variables and Export
  variables are Svelte until S10. They portal to the body, so where they stand in the page does not matter. If the React chunk does
  not load, the Svelte panel (with its own dialogs, the manage view through the same switch) is drawn instead. The host element draws
  nothing of its own (`display: contents`). It goes with the switch when the last slice removes the Svelte shell.
-->
<script lang="ts">
  import type { ComponentProps } from 'svelte';
  import VariablesPanel from '@meldframe/web-panels/VariablesPanel.svelte';
  import ExportVariablesDialog from '@meldframe/web-variables-io/ExportVariablesDialog.svelte';
  import ImportVariablesDialog from '@meldframe/web-variables-io/ImportVariablesDialog.svelte';
  import { reactFor } from './pilot';
  import ManageDialogSwitch from './ManageDialogSwitch.svelte';

  let props: ComponentProps<typeof VariablesPanel> = $props();
  // The React side, island helper included, loads on demand: the editor's bundle does not carry react-dom.
  const react = reactFor('variables', location.search)
    ? Promise.all([import('@meldframe/web-chrome/react/island'), import('@meldframe/web-panels/react/VariablesPanel')]).then(([i, m]) => ({ island: i.island, component: m.VariablesPanel }))
    : null;

  // The dialogs' open state is this page's view state, as it is in VariablesPanel.svelte; the buttons they give the focus back to are in the island.
  let manageOpen = $state(false);
  let importOpen = $state(false);
  let host = $state<HTMLElement | null>(null);
  const button = (id: string) => () => host?.querySelector<HTMLElement>(`[data-testid=${id}]`);
</script>

{#if react}
  {#await react then r}
    <div
      class="contents"
      bind:this={host}
      use:r.island={{
        component: r.component,
        props: { panel: props.panel, target: props.target, dispatch: props.dispatch, onManage: () => (manageOpen = true), onImport: () => (importOpen = true) },
      }}
    ></div>
    <ImportVariablesDialog open={importOpen} onOpenChange={(o) => (importOpen = o)} result={props.io?.import ?? null} dispatch={props.dispatch} opener={button('var-import')} />
    <ExportVariablesDialog dialog={props.io?.export ?? null} dispatch={props.dispatch} opener={button('var-export')} />
    <ManageDialogSwitch open={manageOpen} onOpenChange={(o) => (manageOpen = o)} panel={props.panel} manage={props.manage} events={props.events ?? []} dispatch={props.dispatch} opener={button('var-manage')} />
  {:catch}
    <!-- the React chunk did not load: the Svelte panel is in the entry (it draws its own dialogs), so the variables are still there -->
    <VariablesPanel {...props} manageView={ManageDialogSwitch} />
  {/await}
{:else}
  <VariablesPanel {...props} manageView={ManageDialogSwitch} />
{/if}
