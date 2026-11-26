<!--
  The thin Svelte host of the React pilot (docs/react-web-plan.md, section 7): one element that holds one React root
  (the island helper of web-chrome) and draws a React component with no props. Transitional: it goes with the switch
  in pilot.ts when the last slice removes the Svelte shell.
-->
<script lang="ts">
  import type { ComponentType } from 'react';
  import { island } from '@meldframe/web-chrome/react/island';

  let { component }: { component: ComponentType } = $props();
</script>

<div use:island={{ component, props: {} }}></div>
