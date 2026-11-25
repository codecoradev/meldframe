<!--
  The temporary switch of the shortcut sheet (docs/react-web-plan.md, section 7, slice S4): what the Layout draws in place
  of ShortcutHint.svelte, through `shell.ts`. Without `/?react=palette` it draws the Svelte sheet; with it, the React
  sheet in an island, loaded on demand. The island's host takes no box (`contents`), so the sheet is a child of the
  Layout's wrapper, as the Svelte sheet is. It goes with the switch when the last slice removes the Svelte shell.
-->
<script lang="ts">
  import type { ComponentProps } from 'svelte';
  import ShortcutHint from '@meldframe/web-shell/ShortcutHint.svelte';
  import { reactFor } from './pilot';

  let props: ComponentProps<typeof ShortcutHint> = $props();
  const react = reactFor('palette', location.search)
    ? Promise.all([import('@meldframe/web-chrome/react/island'), import('@meldframe/web-shell/react/ShortcutHint')]).then(([i, m]) => ({ island: i.island, component: m.ShortcutHint }))
    : null;
</script>

{#if react}
  {#await react then r}
    <div class="contents" use:r.island={{ component: r.component, props: { ...props } }}></div>
  {/await}
{:else}
  <ShortcutHint {...props} />
{/if}
