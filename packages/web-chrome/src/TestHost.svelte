<!-- Test only: renders a component under the TooltipProvider that the Shell gives the real controls. -->
<script lang="ts">
  import type { Component } from 'svelte';
  import TooltipProvider from './TooltipProvider.svelte';

  // eslint-disable-next-line @typescript-eslint/no-explicit-any -- any component, whatever its props
  let { of: Of, props }: { of: Component<any>; props: Record<string, unknown> } = $props();
</script>

<TooltipProvider><Of {...props} /></TooltipProvider>
