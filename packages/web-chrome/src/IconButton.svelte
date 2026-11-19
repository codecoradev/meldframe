<!--
  A button that shows an icon only. `label` is required: it becomes the accessible name and the tooltip. `pressed`
  makes it a toggle (aria-pressed) drawn selected: a fill and a 1 px border-strong outline, because the fill alone is about 1.3:1 against a card. Sizes 24, 28, 32 and 36 come from the size tokens.
-->
<script lang="ts">
  import Icon, { type IconName } from './Icon.svelte';
  import Tooltip from './Tooltip.svelte';
  import { composeProps } from './compose';
  import { disabled as off, requireLabel, ring, square, type ControlSize } from './focus';

  interface Props {
    label: string;
    icon: IconName;
    size?: ControlSize;
    shortcut?: string;
    pressed?: boolean;
    disabled?: boolean;
    onclick?: (e: MouseEvent) => void;
    /** Props a wrapping trigger (a menu) hands to its button. */
    triggerProps?: Record<string, unknown>;
    [data: `data-${string}`]: string | undefined;
  }
  let { label, icon, size = 32, shortcut, pressed, disabled = false, onclick, triggerProps = {}, ...rest }: Props = $props();
  const name = $derived(requireLabel(label, 'IconButton'));
</script>

<Tooltip label={name} {shortcut} side="bottom">
  {#snippet trigger(tip)}
    <button
      {...composeProps(tip, triggerProps, rest, { onclick })}
      type="button"
      aria-label={name}
      aria-pressed={pressed}
      {disabled}
      class="{square[size]} {ring} {off} flex shrink-0 items-center justify-center rounded-md border {pressed
        ? 'border-border-strong bg-control text-text-strong act:bg-control-selected'
        : 'border-transparent text-text-muted hov:bg-control hov:text-text-strong act:bg-control-selected'}"
    ><Icon name={icon} /></button>
  {/snippet}
</Tooltip>
