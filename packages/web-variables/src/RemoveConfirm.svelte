<!--
  The question before a removal: what will happen, in numbers, and two answers. The safe one (Cancel) takes the focus
  when this opens; Escape cancels from anywhere inside it and does not close the dialog around it. The words are the
  removal words (`removalWords`); what a click does is the parent's.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import Button from '@meldframe/web-chrome/Button.svelte';
  import type { RemovalWords } from './manage';

  interface Props {
    plan: RemovalWords;
    onConfirm: () => void;
    onCancel: () => void;
  }
  let { plan, onConfirm, onCancel }: Props = $props();
  let root = $state<HTMLElement | null>(null);
  onMount(() => root?.querySelector<HTMLElement>('[data-testid=remove-cancel]')?.focus());
</script>

<!-- Escape closes the question from any control inside it; the group is not a control itself. -->
<div
  bind:this={root}
  role="alertdialog"
  tabindex="-1"
  aria-label="Confirm the removal"
  aria-describedby="remove-confirm-text"
  class="flex flex-col gap-2 rounded-md border border-border bg-surface px-3 py-2"
  data-testid="remove-confirm"
  onkeydown={(e) => {
    if (e.key === 'Escape') {
      e.stopPropagation();
      onCancel();
    }
  }}
>
  <p id="remove-confirm-text" class="m-0 text-small text-text break-words">{plan.text}</p>
  <div class="flex gap-2">
    <Button size="sm" onclick={onCancel} data-testid="remove-cancel">Cancel</Button>
    <Button size="sm" variant="destructive" onclick={onConfirm} data-testid="remove-confirm-yes">{plan.confirm}</Button>
  </div>
</div>
