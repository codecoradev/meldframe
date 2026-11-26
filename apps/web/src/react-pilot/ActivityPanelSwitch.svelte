<!--
  The temporary switch of the Activity panel (docs/react-web-plan.md, section 7, slice S8): what Editor.svelte imports in place
  of ActivityPanel.svelte. Without `/?react=activity` it draws the Svelte panel; with it, the React panel in an island, loaded on
  demand so that the editor's bundle does not carry React. If the React chunk does not load, the Svelte panel (which is in the
  entry) is drawn, so that the person never has a right card with no Activity tab. The host element draws nothing of its own
  (`display: contents`), so the panel is laid out as the Svelte one is. It goes with the switch when the last slice removes the
  Svelte shell.
-->
<script lang="ts">
  import type { ComponentProps } from 'svelte';
  import ActivityPanel from '@meldframe/web-activity/ActivityPanel.svelte';
  import { reactFor } from './pilot';

  let props: ComponentProps<typeof ActivityPanel> = $props();
  // The React side, island helper included, loads on demand: the editor's bundle does not carry react-dom.
  const react = reactFor('activity', location.search)
    ? Promise.all([import('@meldframe/web-chrome/react/island'), import('@meldframe/web-activity/react/ActivityPanel')]).then(([i, m]) => ({ island: i.island, component: m.ActivityPanel }))
    : null;
</script>

{#if react}
  {#await react then r}
    <div class="contents" use:r.island={{ component: r.component, props: { ...props } }}></div>
  {:catch}
    <!-- the React chunk did not load: the Svelte panel is in the entry, so the Activity tab still has its panel -->
    <ActivityPanel {...props} />
  {/await}
{:else}
  <ActivityPanel {...props} />
{/if}
