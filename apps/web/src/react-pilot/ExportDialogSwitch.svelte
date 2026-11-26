<!--
  The temporary switch of the Export dialog (docs/react-web-plan.md, section 7, slice S2): what Editor.svelte imports in
  place of ExportDialog.svelte. Without `/?react=export` it draws the Svelte dialog (Bits UI); with it, the React dialog
  (Base UI) in an island, loaded on demand. The dialog portals to the body, so its host element draws nothing. It goes
  with the switch when the last slice removes the Svelte shell.
-->
<script lang="ts">
  import type { ComponentProps } from 'svelte';
  import ExportDialog from '@meldframe/web-export/ExportDialog.svelte';
  import { reactFor } from './pilot';

  let props: ComponentProps<typeof ExportDialog> = $props();
  // The React side, island helper included, loads on demand: the editor's bundle does not carry react-dom.
  const react = reactFor('export', location.search)
    ? Promise.all([import('@meldframe/web-chrome/react/island'), import('@meldframe/web-export/react/ExportDialog')]).then(([i, m]) => ({ island: i.island, component: m.ExportDialog }))
    : null;
</script>

{#if react}
  {#await react then r}
    <div class="contents" use:r.island={{ component: r.component, props: { ...props } }}></div>
  {:catch}
    <!-- the React chunk did not load: the Svelte dialog is in the entry, so Export still opens -->
    <ExportDialog {...props} />
  {/await}
{:else}
  <ExportDialog {...props} />
{/if}
