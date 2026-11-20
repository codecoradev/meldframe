<!--
  Banners over the top of the canvas: the connection is down, the page and the server disagree, the canvas could not
  start. An error goes in a live region that interrupts (alert) and a warning in one that waits (status). Each may have one action; the page decides what its id does.
  The stack takes no pointer events, so the canvas under it still works. The text is the caller's, drawn as plain text.
-->
<script lang="ts">
  import Button from '@meldframe/web-chrome/Button.svelte';

  export interface Banner {
    id: string;
    tone: 'warning' | 'danger';
    text: string;
    action?: { id: string; label: string };
  }
  interface Props {
    banners: readonly Banner[];
    onAction: (banner: string, action: string) => void;
  }
  let { banners, onAction }: Props = $props();
  const tones = { warning: 'bg-warning-bg text-warning', danger: 'bg-danger-bg text-danger-text' } as const;
</script>

{#snippet item(b: Banner)}
  <div data-testid="banner-{b.id}" data-tone={b.tone} class="pointer-events-auto mb-2 flex max-w-xl items-center gap-3 rounded-md py-2 pr-2 pl-4 text-small shadow-float {tones[b.tone]}">
    <span class="min-w-0 flex-1 wrap-anywhere">{b.text}</span>
    {#if b.action}
      {@const a = b.action}
      <Button size="sm" onclick={() => onAction(b.id, a.id)}>{a.label}</Button>
    {/if}
  </div>
{/snippet}

<!-- Both regions are always on the page and the banners are put into them: a region that appears together with its text is not announced. The alert is for what must interrupt (an error), the status for the rest. -->
<div class="pointer-events-none absolute inset-x-0 top-4 flex flex-col items-center px-4">
  <div role="alert" data-testid="banner-region-alert" class="flex flex-col items-center">
    {#each banners.filter((b) => b.tone === 'danger') as b (b.id)}{@render item(b)}{/each}
  </div>
  <div role="status" data-testid="banner-region-status" class="flex flex-col items-center">
    {#each banners.filter((b) => b.tone !== 'danger') as b (b.id)}{@render item(b)}{/each}
  </div>
</div>
