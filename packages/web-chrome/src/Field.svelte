<!--
  A field: a labelled text input with an optional prefix and suffix. `label` is the accessible name (the prefix is a
  shorthand such as X or W). Hover, focus, invalid and variable-bound outlines follow the state matrix; the ring shows
  when the input has focus. The value is the caller's: it reports input and commits on Enter or blur, Escape puts the
  value back, and after a commit the input shows the caller's value again (a value the caller did not take is undone).
  The keys typed here are text, not commands of the page.
-->
<script lang="ts">
  import { tick, type Snippet } from 'svelte';
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
    /** Something drawn before the input, such as a colour swatch. */
    lead?: Snippet;
    /** The value in the mono face (numbers, ids, variable values). */
    mono?: boolean;
    /** Shown while the field is empty, such as `Mixed` for a value the parts of a whole do not share. */
    placeholder?: string;
    /** `data-testid` of the input. */
    testId?: string;
    [data: `data-${string}`]: string | undefined;
  }
  let { label, value, onInput, onCommit, prefix, suffix, size = 'default', disabled = false, invalid = false, bound = false, lead, mono = false, placeholder, testId, ...rest }: Props = $props();
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
  {#if lead}{@render lead()}{/if}
  <input
    data-testid={testId}
    class="{mono ? 'font-mono' : 'font-[inherit]'} min-w-0 flex-1 border-0 bg-transparent p-0 text-[inherit] outline-none"
    aria-label={name}
    aria-invalid={invalid || undefined}
    {value}
    {placeholder}
    {disabled}
    oninput={(e) => onInput?.(e.currentTarget.value)}
    onkeydown={(e) => {
      const el = e.currentTarget;
      if (e.key === 'Enter') el.blur();
      else if (e.key === 'Escape') {
        el.value = value;
        el.blur();
      }
      e.stopPropagation();
    }}
    onchange={(e) => {
      const el = e.currentTarget;
      onCommit?.(el.value);
      void tick().then(() => (el.value = value));
    }}
  />
  {#if suffix}<span class="shrink-0 text-text-muted" aria-hidden="true">{suffix}</span>{/if}
</label>
