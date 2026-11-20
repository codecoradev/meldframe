<!--
  The bind button of a field and its picker, as the Inspector board draws it: the variable button at the end of a field opens
  a popover with a search and the variables that fit the field (the app layer lists them, only while it is open).
  A row binds the field and the app layer closes the popover (it stays open when the bind is refused). What is inside is
  PickerList. It holds no state of its own: it is open while the
  app state says so.
-->
<script lang="ts">
  import { Popover } from 'bits-ui';
  import type { Action, BindField, Picker, Properties } from '@meldframe/protocol';
  import IconButton from '@meldframe/web-chrome/IconButton.svelte';
  import { closePickerAction, openPickerAction } from '@meldframe/web-document/picker';
  import PickerList from './PickerList.svelte';
  import { FIELD_LABEL } from './variables';

  interface Props {
    properties: Properties;
    field: BindField;
    /** The picker of the app state; this one is open when it is the picker of this field of this node. */
    picker: Picker | null;
    dispatch: (a: Action) => void;
  }
  let { properties: p, field, picker, dispatch }: Props = $props();
  const mine = $derived(picker && picker.id === p.id && picker.field === field ? picker : null);
</script>

<Popover.Root open={!!mine} onOpenChange={(o) => dispatch(o ? openPickerAction(p, field) : closePickerAction())}>
  <Popover.Trigger>
    {#snippet child({ props })}
      <IconButton label="Bind {FIELD_LABEL[field]} to a variable" icon="variable" size={24} triggerProps={props} onclick={props.onclick as (e: MouseEvent) => void} data-testid="prop-{field}-bind" />
    {/snippet}
  </Popover.Trigger>
  <Popover.Portal>
    <Popover.Content side="bottom" align="end" sideOffset={8} data-testid="picker" class="z-50 flex w-72 flex-col overflow-hidden rounded-lg border border-menu-border bg-surface-raised text-small text-text shadow-popover">
      {#if mine}<PickerList picker={mine} {dispatch} />{/if}
    </Popover.Content>
  </Popover.Portal>
</Popover.Root>
