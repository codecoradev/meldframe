<!--
  The thin Svelte host of one React component of web-feedback (the error page): one element that holds one React root
  (the island helper of web-chrome). `display: contents` leaves the layout to the component. Transitional: it goes with
  the switch in feedback.ts when the last slice removes the Svelte shell.
-->
<script lang="ts">
  import type { ComponentType } from 'react';
  import { island } from '@meldframe/web-chrome/react/island';

  // eslint-disable-next-line @typescript-eslint/no-explicit-any -- the page picks the component and its props together
  let { component, props }: { component: ComponentType<any>; props: object } = $props();
</script>

<div class="contents" use:island={{ component, props }}></div>
