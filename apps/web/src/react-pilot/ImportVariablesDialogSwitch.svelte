<!--
  The temporary switch of the Import variables dialog (docs/react-web-plan.md, section 7, slice S10): what draws the dialog in place of
  ImportVariablesDialog.svelte, drawn by `Editor.svelte` beside the Export dialog (not inside the Variables panel, which goes when the right card folds). Without `/?react=io` it draws the Svelte dialog (Bits UI); with it, the React dialog (Base UI's Dialog) in an
  island, loaded on demand so that the editor's bundle does not carry React. The island is not torn down while the page lives, so what the
  React dialog keeps (the text of the field) lives as long as the Svelte dialog's does. The dialog portals to the body, so its host element
  draws nothing (`display: contents`). If the React chunk does not load, the Svelte dialog is drawn instead, and Import variables still opens.
  It goes with the switch when the last slice removes the Svelte shell.
-->
<script lang="ts">
  import type { ComponentProps } from 'svelte';
  import ImportVariablesDialog from '@meldframe/web-variables-io/ImportVariablesDialog.svelte';
  import { reactFor } from './pilot';

  let props: ComponentProps<typeof ImportVariablesDialog> = $props();
  // The React side, island helper included, loads on demand: the editor's bundle does not carry react-dom.
  const react = reactFor('io', location.search)
    ? Promise.all([import('@meldframe/web-chrome/react/island'), import('@meldframe/web-variables-io/react/ImportDialog')]).then(([i, m]) => ({ island: i.island, component: m.ImportDialog }))
    : null;
</script>

{#if react}
  {#await react then r}
    <div class="contents" use:r.island={{ component: r.component, props: { ...props } }}></div>
  {:catch}
    <!-- the React chunk did not load: the Svelte dialog is in the entry, so Import variables still opens -->
    <ImportVariablesDialog {...props} />
  {/await}
{:else}
  <ImportVariablesDialog {...props} />
{/if}
