<!--
  A menu behind an icon button (Bits UI): rows with a label, an optional shortcut, and a disabled state. It holds no
  state: a row calls `onSelect`. The trigger is an IconButton, so the label is its name and tooltip.
-->
<script lang="ts">
  import { DropdownMenu } from 'bits-ui';
  import IconButton from './IconButton.svelte';
  import type { IconName } from './Icon.svelte';
  import type { ControlSize } from './focus';

  interface Item {
    id: string;
    label: string;
    onSelect: () => void;
    disabled?: boolean;
    shortcut?: string;
    /** `data-testid` of the row. */
    testId?: string;
  }
  interface Props {
    label: string;
    icon: IconName;
    items: Item[];
    size?: ControlSize;
    testId?: string;
    side?: 'top' | 'bottom' | 'left' | 'right';
  }
  let { label, icon, items, size = 32, testId, side = 'bottom' }: Props = $props();
</script>

<DropdownMenu.Root>
  <DropdownMenu.Trigger>
    {#snippet child({ props })}<IconButton {label} {icon} {size} triggerProps={props} data-testid={testId} />{/snippet}
  </DropdownMenu.Trigger>
  <DropdownMenu.Portal>
    <DropdownMenu.Content data-owns-keys {side} sideOffset={8} align="start" class="z-50 min-w-48 rounded-lg border border-menu-border bg-surface-raised p-1 text-small text-text shadow-popover">
      {#each items as it (it.id)}
        <DropdownMenu.Item disabled={it.disabled} onSelect={it.onSelect} data-testid={it.testId} class="flex h-(--mf-size-control-lg) cursor-default items-center justify-between gap-4 rounded-sm px-3 outline-none data-disabled:opacity-(--mf-opacity-disabled) data-highlighted:bg-control">
          {it.label}{#if it.shortcut}<span class="font-mono text-caption text-text-muted">{it.shortcut}</span>{/if}
        </DropdownMenu.Item>
      {/each}
    </DropdownMenu.Content>
  </DropdownMenu.Portal>
</DropdownMenu.Root>
