<!--
  A checkbox with its label, as the Properties board draws it (Clip content): a 16 px box that fills with the accent
  and shows a check. A native checkbox underneath, so the keyboard and the screen reader work as they do everywhere.
-->
<script lang="ts">
  import Icon from './Icon.svelte';
  import { dim, ringWithin, requireLabel } from './focus';

  interface Props {
    label: string;
    checked: boolean;
    onChange: (checked: boolean) => void;
    disabled?: boolean;
    [data: `data-${string}`]: string | undefined;
  }
  let { label, checked, onChange, disabled = false, ...rest }: Props = $props();
  const name = $derived(requireLabel(label, 'Checkbox'));
</script>

<label class="{disabled ? dim : ''} flex h-(--mf-size-control-md) cursor-pointer items-center gap-2 text-small text-text-secondary">
  <span class="{ringWithin} relative flex size-4 shrink-0 items-center justify-center rounded-sm border {checked ? 'border-transparent bg-accent-strong text-on-accent' : 'border-border-strong bg-field'}">
    <input {...rest} type="checkbox" class="absolute inset-0 m-0 cursor-pointer opacity-0" {checked} {disabled} onchange={(e) => onChange(e.currentTarget.checked)} />
    {#if checked}<Icon name="check" />{/if}
  </span>
  {name}
</label>
