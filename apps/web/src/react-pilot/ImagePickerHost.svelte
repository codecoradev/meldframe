<!--
  The Svelte image picker of the Fill section, for the React Properties panel (docs/react-web-plan.md, section 7, slice S6): the
  picker is Svelte until S7 and the React panel draws it in a ForeignSlot, where it is mounted on an element of its own and so is
  outside the tree that gives the controls their tooltip provider. This gives it one. It goes with S7.
-->
<script lang="ts">
  import type { ComponentProps } from 'svelte';
  import TooltipProvider from '@meldframe/web-chrome/TooltipProvider.svelte';
  import ImageFillPicker from '@meldframe/web-panels/ImageFillPicker.svelte';

  let props: ComponentProps<typeof ImageFillPicker> = $props();
</script>

<TooltipProvider><ImageFillPicker {...props} /></TooltipProvider>
