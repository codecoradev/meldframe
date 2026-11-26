<!--
  The temporary switch of the Export section (docs/react-web-plan.md, section 7, slice S2): what Editor.svelte imports in
  place of ExportSection.svelte. Without `/?react=export` it draws the Svelte section; with it, the React section in an
  island, loaded on demand so that the editor's bundle does not carry React. The host element holds the rule above the
  section (the section is the first child of its own host, so its `first:border-t-0` would drop it) and is hidden while
  the section draws nothing. It goes with the switch when the last slice removes the Svelte shell.
-->
<script lang="ts">
  import type { ComponentProps } from 'svelte';
  import ExportSection from '@meldframe/web-export/ExportSection.svelte';
  import { reactFor } from './pilot';

  let props: ComponentProps<typeof ExportSection> = $props();
  // The React side, island helper included, loads on demand: the editor's bundle does not carry react-dom.
  const react = reactFor('export', location.search)
    ? Promise.all([import('@meldframe/web-chrome/react/island'), import('@meldframe/web-export/react/ExportSection')]).then(([i, m]) => ({ island: i.island, component: m.ExportSection }))
    : null;
</script>

{#if react}
  {#await react then r}
    <div class="border-t border-hairline empty:hidden" use:r.island={{ component: r.component, props: { ...props } }}></div>
  {:catch}
    <!-- the React chunk did not load: the Svelte section is in the entry, so Export as PNG is still offered -->
    <ExportSection {...props} />
  {/await}
{:else}
  <ExportSection {...props} />
{/if}
