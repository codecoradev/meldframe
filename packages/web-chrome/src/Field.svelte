<!--
  A field: a labelled text input with an optional prefix and suffix. `label` is the accessible name (the prefix is a
  shorthand such as X or W). Hover, focus, invalid and variable-bound outlines follow the state matrix; the ring shows
  when the input has focus. The value is the caller's: it reports input and commits on Enter or blur.
-->
<script lang="ts">
  import { dim, fieldHeight, requireLabel, ringWithin, type FieldSize } from './focus';

  interface Props {
    label: string;
    value: string;
    onInput?: (value: string) => void;
    onCommit?: (value: string) => void;
    prefix?: string;
    suffix?: string;
    size?: FieldSize;
    disabled?: boolean;
    invalid?: boolean;
    /** Bound to a Variable: chip colours, the name is the value. */
    bound?: boolean;
    [data: `data-${string}`]: string | undefined;
  }
  let { label, value, onInput, onCommit, prefix, suffix, size = 'default', disabled = false, invalid = false, bound = false, ...rest }: Props = $props();
  const name = $derived(requireLabel(label, 'Field'));
  const look = $derived(
    invalid
      ? 'border-danger bg-danger-bg text-danger-text'
      : bound
        ? 'border-transparent bg-chip-bg text-chip-text hov:border-chip-border'
        : 'border-divider bg-field text-text hov:border-border-strong foc-in:border-selection',
  );
</script>

<label {...rest} class="{fieldHeight[size]} {ringWithin} {disabled ? dim : ''} flex min-w-0 items-center gap-2 rounded-md border px-3 text-small {look}">
  {#if prefix}<span class="shrink-0 text-text-muted" aria-hidden="true">{prefix}</span>{/if}
  <input
    class="min-w-0 flex-1 border-0 bg-transparent p-0 font-[inherit] text-[inherit] outline-none"
    aria-label={name}
    aria-invalid={invalid || undefined}
    {value}
    {disabled}
    oninput={(e) => onInput?.(e.currentTarget.value)}
    onchange={(e) => onCommit?.(e.currentTarget.value)}
  />
  {#if suffix}<span class="shrink-0 text-text-muted" aria-hidden="true">{suffix}</span>{/if}
</label>
