<!--
  A menu behind an icon button (Bits UI): rows with a label, an optional shortcut, and a disabled state. It holds no
  state: a row calls `onSelect`. The trigger is an IconButton, so the label is its name and tooltip. With `text` the
  trigger is a plain text button instead (the zoom readout): the text is what it shows and the label its name, which
  repeats the text.
-->
<script lang="ts">
  import { DropdownMenu } from 'bits-ui';
  import IconButton from './IconButton.svelte';
  import type { IconName } from './Icon.svelte';
  import { ring, rememberFocus, type ControlSize } from './focus';

  interface Item {
    id: string;
    label: string;
    onSelect: () => void;
    disabled?: boolean;
    shortcut?: string;
    /** `data-testid` of the row. */
    testId?: string;
    /** The row opens a dialog, which takes the focus: the menu does not give it back to its button. */
    opensDialog?: boolean;
  }
  interface Props {
    label: string;
    icon?: IconName;
    /** Shows this text as the trigger instead of the icon. */
    text?: string;
    items: Item[];
    size?: ControlSize;
    testId?: string;
    side?: 'top' | 'bottom' | 'left' | 'right';
    align?: 'start' | 'center' | 'end';
  }
  let { label, icon = 'menu', text, items, size = 32, testId, side = 'bottom', align = 'start' }: Props = $props();
  let keepFocus = false;
  let trigger = $state<HTMLElement | null>(null);
</script>

<DropdownMenu.Root>
  <DropdownMenu.Trigger bind:ref={trigger}>
    {#snippet child({ props })}
      {#if text}<button {...props} type="button" aria-label={label} data-testid={testId} class="{ring} h-(--mf-size-control-lg) rounded-md border border-transparent px-2 font-mono text-small text-text-muted hov:bg-control hov:text-text-strong act:bg-control-selected">{text}</button>
      {:else}<IconButton {label} {icon} {size} triggerProps={props} data-testid={testId} />{/if}
    {/snippet}
  </DropdownMenu.Trigger>
  <DropdownMenu.Portal>
    <DropdownMenu.Content data-owns-keys onCloseAutoFocus={(e) => { if (keepFocus) e.preventDefault(); keepFocus = false; }} {side} sideOffset={8} {align} class="z-50 min-w-48 rounded-lg border border-menu-border bg-surface-raised p-1 text-small text-text shadow-popover">
      {#each items as it (it.id)}
        <DropdownMenu.Item disabled={it.disabled} onSelect={() => { keepFocus = !!it.opensDialog; if (keepFocus) rememberFocus(trigger); it.onSelect(); }} data-testid={it.testId} class="flex h-(--mf-size-control-lg) cursor-default items-center justify-between gap-4 rounded-sm px-3 outline-none data-disabled:opacity-(--mf-opacity-disabled) data-highlighted:bg-control">
          {it.label}{#if it.shortcut}<span class="font-mono text-caption text-text-muted">{it.shortcut}</span>{/if}
        </DropdownMenu.Item>
      {/each}
    </DropdownMenu.Content>
  </DropdownMenu.Portal>
</DropdownMenu.Root>
