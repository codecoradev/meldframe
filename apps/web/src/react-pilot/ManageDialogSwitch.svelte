<!--
  The temporary switch of the manage view (docs/react-web-plan.md, section 7, slice S9): what draws the Variables manage view in place
  of ManageDialog.svelte, through `VariablesPanelSwitch.svelte` (the Svelte panel takes it as `manageView`; the React panel's switch draws
  it beside the island). Without `/?react=manage` it draws the Svelte dialog (Bits UI); with it, the React dialog (Base UI's Dialog) in an
  island, loaded on demand so that the editor's bundle does not carry React. The island draws every update to its end (`syncIsland.tsx`):
  the dialog reads the events of each snapshot, and with the shared helper the events of a snapshot were lost (inferred: React keeping the
  last of two updates). Delete `syncIsland.tsx` and import `island` from the shared helper when the shared island flush lands (S8,
  fix/web-island-flush). The
  dialog portals to the body, so its host element draws nothing (`display: contents`). If the React chunk does not load, the Svelte dialog is drawn instead, and the manage view still opens.
  It goes with the switch when the last slice removes the Svelte shell.
-->
<script lang="ts">
  import type { ComponentProps } from 'svelte';
  import ManageDialog from '@meldframe/web-variables/ManageDialog.svelte';
  import { reactFor } from './pilot';

  let props: ComponentProps<typeof ManageDialog> = $props();
  // The React side, island helper included, loads on demand: the editor's bundle does not carry react-dom.
  const react = reactFor('manage', location.search)
    ? Promise.all([import('./syncIsland'), import('@meldframe/web-variables/react/ManageDialog')]).then(([i, m]) => ({ island: i.syncIsland, component: m.ManageDialog }))
    : null;
</script>

{#if react}
  {#await react then r}
    <div class="contents" use:r.island={{ component: r.component, props: { ...props } }}></div>
  {:catch}
    <!-- the React chunk did not load: the Svelte dialog is in the entry, so the manage view still opens -->
    <ManageDialog {...props} />
  {/await}
{:else}
  <ManageDialog {...props} />
{/if}
