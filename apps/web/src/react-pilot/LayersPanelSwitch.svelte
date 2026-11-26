<!--
  The temporary switch of the Layers panel (docs/react-web-plan.md, section 7, slice S5): what Editor.svelte imports in place
  of LayersPanel.svelte. Without `/?react=layers` it draws the Svelte panel; with it, the React panel in an island, loaded on
  demand so that the editor's bundle does not carry React. If the React chunk does not load, the Svelte panel (which is in the
  entry) is drawn, so that the person never has a page with no layers. The host element draws nothing of its own
  (`display: contents`), so the panel is laid out as the Svelte one is. It goes with the switch when the last slice removes
  the Svelte shell.
-->
<script lang="ts">
  import type { ComponentProps } from 'svelte';
  import LayersPanel from '@meldframe/web-panels/LayersPanel.svelte';
  import { reactFor } from './pilot';

  let props: ComponentProps<typeof LayersPanel> = $props();
  // The React side, island helper included, loads on demand: the editor's bundle does not carry react-dom.
  const react = reactFor('layers', location.search)
    ? Promise.all([import('@meldframe/web-chrome/react/island'), import('@meldframe/web-panels/react/LayersPanel')]).then(([i, m]) => ({ island: i.island, component: m.LayersPanel }))
    : null;
</script>

{#if react}
  {#await react then r}
    <div class="contents" use:r.island={{ component: r.component, props: { ...props } }}></div>
  {:catch}
    <!-- the React chunk did not load: the Svelte panel is in the entry, so the layers are still there -->
    <LayersPanel {...props} />
  {/await}
{:else}
  <LayersPanel {...props} />
{/if}
