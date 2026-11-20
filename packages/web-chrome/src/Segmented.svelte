<!--
  A segmented control: one choice of a few, as a radio group. Each option has a `label` (the accessible name; shown
  unless the option is icon-only). Arrow keys, Home and End move the choice; only the chosen option is in the Tab order.
-->
<script lang="ts">
  import Icon, { type IconName } from './Icon.svelte';
  import { dim, nextIndex, requireLabel, ring } from './focus';

  interface Option {
    value: string;
    label: string;
    icon?: IconName;
  }
  interface Props {
    label: string;
    options: Option[];
    value: string;
    onChange: (value: string) => void;
    disabled?: boolean;
    /** Show the icon alone; the label stays as the name. */
    iconOnly?: boolean;
    [data: `data-${string}`]: string | undefined;
  }
  let { label, options, value, onChange, disabled = false, iconOnly = false, ...rest }: Props = $props();
  const group = $derived(requireLabel(label, 'Segmented'));
  const current = $derived(Math.max(0, options.findIndex((o) => o.value === value)));
  let host: HTMLElement | undefined = $state();

  function onkeydown(e: KeyboardEvent, i: number) {
    const to = nextIndex(e.key, i, options.length);
    if (to === undefined) return;
    e.preventDefault();
    onChange(options[to]!.value);
    (host?.querySelectorAll<HTMLElement>('[role=radio]')[to])?.focus();
  }
</script>

<div bind:this={host} {...rest} role="radiogroup" aria-label={group} aria-disabled={disabled || undefined} class="{disabled ? dim : ''} flex gap-0.5 rounded-md bg-field p-0.5">
  {#each options as o, i (o.value)}
    {@const on = o.value === value}
    <button
      type="button"
      role="radio"
      aria-checked={on}
      aria-label={iconOnly ? o.label : undefined}
      {disabled}
      tabindex={i === current ? 0 : -1}
      onclick={() => onChange(o.value)}
      onkeydown={(e) => onkeydown(e, i)}
      class="{ring} flex h-(--mf-size-control-md) min-w-0 flex-1 items-center justify-center gap-1 rounded-sm px-3 text-small {on
        ? 'border border-border-strong bg-control-selected font-semibold text-text-strong'
        : 'border border-transparent text-text-muted hov:text-text'}"
      title={o.label}
    >{#if o.icon}<Icon name={o.icon} />{/if}{#if !iconOnly}<span class="min-w-0 truncate">{o.label}</span>{/if}</button>
  {/each}
</div>
