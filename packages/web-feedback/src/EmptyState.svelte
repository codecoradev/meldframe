<!--
  An empty state over the canvas: what is missing, what to do about it, and at most one button. The container takes no
  pointer events, so the canvas under it still works (a drag with the Frame tool starts anywhere); only the card does.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    title: string;
    text: string;
    children?: Snippet;
  }
  let { title, text, children }: Props = $props();
</script>

<div class="pointer-events-none absolute inset-0 grid place-items-center p-8">
  <section data-testid="empty-state" aria-label={title} class="pointer-events-auto flex max-w-80 flex-col items-center gap-3 rounded-lg border border-border bg-surface p-6 text-center shadow-float">
    <h2 class="m-0 text-title font-semibold text-text-strong">{title}</h2>
    <p class="m-0 text-body text-text-secondary">{text}</p>
    {#if children}{@render children()}{/if}
  </section>
</div>
