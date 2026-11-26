<!--
  The temporary switch of the command palette (docs/react-web-plan.md, section 7, slice S4): what the Layout draws in place
  of Palette.svelte, through `shell.ts`. Without `/?react=palette` it draws the Svelte palette (Bits UI); with it, the
  React palette (Base UI) in an island, loaded on demand. The dialog portals to the body, so its host element draws
  nothing. It goes with the switch when the last slice removes the Svelte shell.
-->
<script lang="ts">
  import type { ComponentProps } from 'svelte';
  import Palette from '@meldframe/web-shell/Palette.svelte';
  import { reactFor } from './pilot';

  let props: ComponentProps<typeof Palette> = $props();
  // The React side, island helper included, loads on demand: the editor's bundle does not carry react-dom.
  const react = reactFor('palette', location.search)
    ? Promise.all([import('@meldframe/web-chrome/react/island'), import('@meldframe/web-shell/react/Palette')]).then(([i, m]) => ({ island: i.island, component: m.Palette }))
    : null;
</script>

{#if react}
  {#await react then r}
    <div class="contents" use:r.island={{ component: r.component, props: { ...props } }}></div>
  {:catch}
    <!-- the React chunk did not load: the Svelte palette is in the entry, so Ctrl+K still opens a palette -->
    <Palette {...props} />
  {/await}
{:else}
  <Palette {...props} />
{/if}
