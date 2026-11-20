<!--
  The toast: a short message over the bottom of the canvas (a refusal, a problem with an image). The live region is
  always on the page and the text is put into it, because a region that appears together with its text is not
  announced, and each message is a new element (a key on its id), so the same text said twice is read again. A message goes away by itself (`Toaster`), and the button takes it away sooner. The region takes no
  pointer events, so the canvas under it still works. The text is the caller's, drawn as plain text.
-->
<script lang="ts">
  import IconButton from '@meldframe/web-chrome/IconButton.svelte';
  import type { ToastView } from './toaster';

  interface Props {
    toast: ToastView | null;
    onClose: () => void;
  }
  let { toast, onClose }: Props = $props();
  // The pairs of DESIGN.md 3.2: the warning text on its fill, the pale error text on its fill.
  const tones = { warning: 'bg-warning-bg text-warning', danger: 'bg-danger-bg text-danger-text' } as const;
</script>

<div role="status" data-testid="toast-region" class="pointer-events-none absolute inset-x-0 bottom-24 flex justify-center px-4">
  {#if toast}
    {#key toast.id}
      <div data-testid="notice" data-tone={toast.tone} class="pointer-events-auto flex max-w-96 items-center gap-2 rounded-md py-1 pr-1 pl-4 text-small shadow-float {tones[toast.tone]}">
        <span class="min-w-0 flex-1 py-1 wrap-anywhere">{toast.text}</span>
        <IconButton label="Dismiss" icon="close" size={24} onclick={onClose} />
      </div>
    {/key}
  {/if}
</div>
