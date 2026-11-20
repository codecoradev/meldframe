<!--
  A tooltip on hover and on keyboard focus, never a standing element. The trigger is a snippet so that the control
  itself (a button) is the trigger and keeps its own name; the tooltip repeats it and adds the shortcut.
-->
<script lang="ts">
  import { Tooltip } from 'bits-ui';
  import type { Snippet } from 'svelte';

  interface Props {
    label: string;
    shortcut?: string;
    side?: 'top' | 'bottom' | 'left' | 'right';
    trigger: Snippet<[Record<string, unknown>]>;
  }
  let { label, shortcut, side = 'bottom', trigger }: Props = $props();
</script>

<!-- One TooltipProvider above the controls (the Shell, the matrix page) gives the neighbours their shared delay. -->
<Tooltip.Root>
  <Tooltip.Trigger>{#snippet child({ props })}{@render trigger(props)}{/snippet}</Tooltip.Trigger>
  <Tooltip.Portal>
    <Tooltip.Content role="tooltip" {side} sideOffset={8} class="z-50 flex items-center gap-2 rounded-md border border-menu-border bg-surface-raised px-2 py-1 text-small text-text shadow-popover">
      {label}{#if shortcut}<span class="font-mono text-text-muted">{shortcut}</span>{/if}
    </Tooltip.Content>
  </Tooltip.Portal>
</Tooltip.Root>
