<!--
  A section of a panel: a title, optional actions on the right, a rule above it. With `onToggle` the title is a button
  that folds the section (aria-expanded); without it the title is plain text. The body is the caller's.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';
  import { ring } from './focus';

  interface Props {
    title: string;
    /** Folding: `open` and the handler that flips it; omit both for a section that stays open. */
    open?: boolean;
    onToggle?: () => void;
    actions?: Snippet;
    children: Snippet;
    [data: `data-${string}`]: string | undefined;
  }
  let { title, open = true, onToggle, actions, children, ...rest }: Props = $props();
</script>

<section {...rest} aria-label={title} class="flex flex-col gap-3 border-t border-hairline px-4 py-4 first:border-t-0">
  <header class="flex items-center justify-between text-body font-semibold text-text-strong">
    {#if onToggle}
      <button type="button" aria-expanded={open} onclick={onToggle} class="{ring} -ml-1 flex h-(--mf-size-control-sm) items-center gap-1 rounded-sm px-1">
        <span class="text-icon-muted"><Icon name={open ? 'chevron-down' : 'chevron-right'} /></span>{title}
      </button>
    {:else}
      <h3 class="m-0 text-[inherit]">{title}</h3>
    {/if}
    {#if actions}<span class="flex">{@render actions()}</span>{/if}
  </header>
  {#if open}{@render children()}{/if}
</section>
