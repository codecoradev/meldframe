<!--
  The thin Svelte host of what the editor lays over the canvas (the empty state, the toast and the banners) when the page
  asks for the React copy (`/?react=feedback`, feedback.ts): the props of the three Svelte components, handed to one
  React root. `display: contents` keeps the layout of the Svelte version: the React elements are absolutely placed in
  the canvas layer as before. Transitional: it goes with the switch when the last slice removes the Svelte shell.
-->
<script lang="ts">
  import { island } from '@meldframe/web-chrome/react/island';
  import { FeedbackLayer, type FeedbackLayerProps } from '@meldframe/web-feedback/react/FeedbackLayer';

  let props: FeedbackLayerProps = $props();
</script>

<div class="contents" use:island={{ component: FeedbackLayer, props: { ...props } }}></div>
