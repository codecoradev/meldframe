<!--
  A text button: primary (accent-strong), secondary (control) or destructive (danger-strong). White text sits on the
  strong fills only (5.17:1); the brand blue carries no text. Hover and pressed fills are the tokens of DESIGN.md.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import { disabled as off, ring } from './focus';

  interface Props {
    variant?: 'primary' | 'secondary' | 'destructive';
    disabled?: boolean;
    onclick?: (e: MouseEvent) => void;
    children: Snippet;
    [data: `data-${string}`]: string | undefined;
  }
  let { variant = 'secondary', disabled = false, onclick, children, ...rest }: Props = $props();
  const fills = {
    primary: 'bg-accent-strong text-on-accent font-semibold hov:bg-accent-hover act:bg-accent-pressed',
    secondary: 'bg-control text-text font-medium hov:bg-control-selected act:bg-field',
    destructive: 'bg-danger-strong text-on-accent font-semibold hov:bg-danger-hover act:bg-danger-pressed',
  };
</script>

<button {...rest} type="button" {disabled} {onclick} class="{ring} {off} h-(--mf-size-control-lg) rounded-md px-4 text-small {fills[variant]}">{@render children()}</button>
