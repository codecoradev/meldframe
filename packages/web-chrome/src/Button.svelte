<!--
  A text button: primary (accent-strong), secondary (control) or destructive (danger-strong). White text sits on the
  strong fills only (5.17:1); the brand blue carries no text. Hover and pressed fills are the tokens of DESIGN.md.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import { disabled as off, ring } from './focus';

  interface Props {
    variant?: 'primary' | 'secondary' | 'destructive';
    /** `sm` is 28 high, for a row of a panel. */
    size?: 'default' | 'sm';
    /** A toggle: aria-pressed, drawn in the variable chip colours when on. */
    pressed?: boolean;
    disabled?: boolean;
    onclick?: (e: MouseEvent) => void;
    /** A name that starts with the visible text, where the text alone does not say what it acts on. */
    'aria-label'?: string;
    /** The id of the text that says why the button is disabled, or what it does beyond its name. */
    'aria-describedby'?: string;
    children: Snippet;
    [data: `data-${string}`]: string | undefined;
  }
  let { variant = 'secondary', size = 'default', pressed, disabled = false, onclick, children, ...rest }: Props = $props();
  const fills = {
    primary: 'bg-accent-strong text-on-accent font-semibold hov:bg-accent-hover act:bg-accent-pressed',
    secondary: 'bg-control text-text font-medium hov:bg-control-selected act:bg-field',
    destructive: 'bg-danger-strong text-on-accent font-semibold hov:bg-danger-hover act:bg-danger-pressed',
  };
</script>

<button {...rest} type="button" aria-pressed={pressed} {disabled} {onclick} class="{ring} {off} {size === 'sm' ? 'h-(--mf-size-control-md) px-3' : 'h-(--mf-size-control-lg) px-4'} rounded-md text-small {pressed ? 'bg-chip-bg font-semibold text-chip-text' : fills[variant]}">{@render children()}</button>
