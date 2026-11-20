<!-- One provider above a group of controls: the first tooltip waits 400 ms, its neighbours then open at once. -->
<script lang="ts">
  import { Tooltip } from 'bits-ui';
  import type { Snippet } from 'svelte';

  let { children }: { children: Snippet } = $props();
</script>

<Tooltip.Provider delayDuration={400} skipDelayDuration={300}>{@render children()}</Tooltip.Provider>
