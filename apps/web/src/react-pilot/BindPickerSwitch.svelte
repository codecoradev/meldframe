<!--
  The temporary switch of the bind picker (docs/react-web-plan.md, section 7, slice S5): what the Properties panel draws in
  place of BindPicker.svelte, through `bindParts` in shell.ts (the panel takes it as `bindView`). Without `/?react=variables`
  it draws the Svelte picker (Bits UI); with it, the React picker (Base UI's Popover) in an island, loaded on demand. The host
  draws nothing of its own (`display: contents`): the button is a flex item of the field's row, as in the Svelte copy. The
  popover portals to the body. If the React chunk does not load, the Svelte picker is drawn instead. It goes with the switch when the last slice removes the Svelte shell.
-->
<script lang="ts">
  import type { ComponentProps } from 'svelte';
  import BindPicker from '@meldframe/web-panels/BindPicker.svelte';
  import { reactFor } from './pilot';

  let props: ComponentProps<typeof BindPicker> = $props();
  // The React side, island helper included, loads on demand: the editor's bundle does not carry react-dom.
  const react = reactFor('variables', location.search)
    ? Promise.all([import('@meldframe/web-chrome/react/island'), import('@meldframe/web-panels/react/BindPicker')]).then(([i, m]) => ({ island: i.island, component: m.BindPicker }))
    : null;
</script>

{#if react}
  {#await react then r}
    <div class="contents" use:r.island={{ component: r.component, props: { ...props } }}></div>
  {:catch}
    <!-- the React chunk did not load: the Svelte picker is in the entry, so the bind button is still there -->
    <BindPicker {...props} />
  {/await}
{:else}
  <BindPicker {...props} />
{/if}
