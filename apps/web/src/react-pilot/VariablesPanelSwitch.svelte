<!--
  The temporary switch of the Variables panel (docs/react-web-plan.md, section 7, slice S5): what Editor.svelte imports in
  place of VariablesPanel.svelte. Without `/?react=variables` it draws the Svelte panel; with it, the React panel in an island,
  loaded on demand so that the editor's bundle does not carry React. The three dialogs of the panel (the manage view, Import
  variables, Export variables) cannot be drawn inside the panel's React root, so this file draws them beside the island, open by the
  app layer's state (`manage.open`, `io.importOpen`, `io.export`; the React panel only has the three buttons, which send the actions). Each is
  a switch of its own, the React dialog with `/?react=manage` (S9) or `/?react=io` (S10, Import variables and Export variables) and
  the Svelte dialog without it. They portal to the body, so where they stand in the page does not matter. If the React chunk of the panel does
  not load, the Svelte panel (with its own dialogs, through the same three switches) is drawn instead. The host element draws
  nothing of its own (`display: contents`). It goes with the switch when the last slice removes the Svelte shell.
-->
<script lang="ts">
  import type { ComponentProps } from 'svelte';
  import VariablesPanel from '@meldframe/web-panels/VariablesPanel.svelte';
  import { reactFor } from './pilot';
  import ExportVariablesDialogSwitch from './ExportVariablesDialogSwitch.svelte';
  import ImportVariablesDialogSwitch from './ImportVariablesDialogSwitch.svelte';
  import ManageDialogSwitch from './ManageDialogSwitch.svelte';

  let props: ComponentProps<typeof VariablesPanel> = $props();
  // The React side, island helper included, loads on demand: the editor's bundle does not carry react-dom.
  const react = reactFor('variables', location.search)
    ? Promise.all([import('@meldframe/web-chrome/react/island'), import('@meldframe/web-panels/react/VariablesPanel')]).then(([i, m]) => ({ island: i.island, component: m.VariablesPanel }))
    : null;

  // The buttons the dialogs give the focus back to are in the island.
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
        props: { panel: props.panel, target: props.target, dispatch: props.dispatch },
      }}
    ></div>
    <ImportVariablesDialogSwitch open={props.io?.importOpen ?? false} result={props.io?.import ?? null} dispatch={props.dispatch} opener={button('var-import')} />
    <ExportVariablesDialogSwitch dialog={props.io?.export ?? null} dispatch={props.dispatch} opener={button('var-export')} />
    <ManageDialogSwitch open={props.manage.open} panel={props.panel} manage={props.manage} events={props.events ?? []} dispatch={props.dispatch} opener={button('var-manage')} />
  {:catch}
    <!-- the React chunk did not load: the Svelte panel is in the entry (it draws its own dialogs), so the variables are still there -->
    <VariablesPanel {...props} manageView={ManageDialogSwitch} importView={ImportVariablesDialogSwitch} exportView={ExportVariablesDialogSwitch} />
  {/await}
{:else}
  <VariablesPanel {...props} manageView={ManageDialogSwitch} importView={ImportVariablesDialogSwitch} exportView={ExportVariablesDialogSwitch} />
{/if}
