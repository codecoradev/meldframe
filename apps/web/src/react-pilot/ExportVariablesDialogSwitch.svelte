<!--
  The temporary switch of the Export variables dialog (docs/react-web-plan.md, section 7, slice S10): what draws the dialog in place of
  ExportVariablesDialog.svelte, through `VariablesPanelSwitch.svelte` (the Svelte panel takes it as `exportView`; the React panel's switch
  draws it beside the island). Without `/?react=io` it draws the Svelte dialog (Bits UI); with it, the React dialog (Base UI's Dialog) in an
  island, loaded on demand so that the editor's bundle does not carry React. The dialog is open while the app layer holds its text, and it
  portals to the body, so its host element draws nothing (`display: contents`). If the React chunk does not load, the Svelte dialog is drawn
  instead, and Export variables still opens. It goes with the switch when the last slice removes the Svelte shell.
-->
<script lang="ts">
  import type { ComponentProps } from 'svelte';
  import ExportVariablesDialog from '@meldframe/web-variables-io/ExportVariablesDialog.svelte';
  import { reactFor } from './pilot';

  let props: ComponentProps<typeof ExportVariablesDialog> = $props();
  // The React side, island helper included, loads on demand: the editor's bundle does not carry react-dom.
  const react = reactFor('io', location.search)
    ? Promise.all([import('@meldframe/web-chrome/react/island'), import('@meldframe/web-variables-io/react/ExportDialog')]).then(([i, m]) => ({ island: i.island, component: m.ExportDialog }))
    : null;
</script>

{#if react}
  {#await react then r}
    <div class="contents" use:r.island={{ component: r.component, props: { ...props } }}></div>
  {:catch}
    <!-- the React chunk did not load: the Svelte dialog is in the entry, so Export variables still opens -->
    <ExportVariablesDialog {...props} />
  {/await}
{:else}
  <ExportVariablesDialog {...props} />
{/if}
