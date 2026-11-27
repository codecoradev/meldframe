<!--
  The manage view in a dialog (Bits UI: the scrim, the focus, Escape, the title and the close button) around
  `ManageBody`. It is opened from the Variables panel and gives the focus back to the button that opened it. Open
  or closed, and the content, are the app state (`state.manage`: `open` is set by `ManageOp::Open` and cleared by `Close`).
-->
<script lang="ts">
  import { Dialog } from 'bits-ui';
  import type { Action, Event, ManageState, VariablesPanel } from '@meldframe/protocol';
  import IconButton from '@meldframe/web-chrome/IconButton.svelte';
  import TooltipProvider from '@meldframe/web-chrome/TooltipProvider.svelte';
  import ManageBody from './ManageBody.svelte';

  interface Props {
    /** `state.manage.open`. */
    open: boolean;
    panel: VariablesPanel;
    /** `AppState.manage`: what the manage view shows and asks. */
    manage: ManageState;
    events: readonly Event[];
    dispatch: (a: Action) => void;
    /** The button that opened the dialog, to give the focus back to. */
    opener: () => HTMLElement | null | undefined;
  }
  let { open, panel, manage, events, dispatch, opener }: Props = $props();

  // Whatever closes the dialog (the Close button, the scrim, Escape) tells the app layer, which closes it and forgets the question, the
  // rename field, the forms and the choice of a collection: nothing open before the close is open after it. Both copies of the
  // Variables panel (Svelte and the React island) draw this dialog, so this is the one place.
  const change = (next: boolean) => {
    if (!next) dispatch({ type: 'manage', op: { type: 'close' } });
  };

  // The first thing to focus is the collection on view, not the Close button: a tooltip opens on that button and takes the first Escape.
  const focusStart = (e: globalThis.Event) => {
    const el = document.querySelector<HTMLElement>('[data-testid=manage-dialog] [aria-current=true]') ?? document.querySelector<HTMLElement>('[data-testid=manage-new-collection]');
    if (el) {
      e.preventDefault();
      el.focus();
    }
  };
  const giveBack = (e: globalThis.Event) => {
    const el = opener();
    if (el?.isConnected) {
      e.preventDefault();
      el.focus();
    }
  };
</script>

<TooltipProvider>
  <Dialog.Root {open} onOpenChange={change}>
    <Dialog.Portal>
      <Dialog.Overlay class="fixed inset-0 z-40 bg-scrim" />
      <Dialog.Content data-owns-keys data-testid="manage-dialog" onOpenAutoFocus={focusStart} onCloseAutoFocus={giveBack} class="fixed inset-8 z-50 m-auto flex h-fit max-h-full min-h-96 w-full max-w-4xl flex-col overflow-hidden rounded-lg border border-divider bg-surface font-sans text-body text-text shadow-dialog">
        <header class="flex h-14 shrink-0 items-center justify-between border-b border-hairline pr-3 pl-5">
          <Dialog.Title class="m-0 truncate text-title font-semibold text-text-strong">Manage variables</Dialog.Title>
          <Dialog.Close>{#snippet child({ props })}<IconButton label="Close" icon="close" triggerProps={props} data-testid="manage-close" />{/snippet}</Dialog.Close>
        </header>
        <Dialog.Description class="sr-only">Add and remove collections, modes and variables. A removal asks first, and one undo takes it back.</Dialog.Description>
        <ManageBody {panel} {manage} {events} {dispatch} />
      </Dialog.Content>
    </Dialog.Portal>
  </Dialog.Root>
</TooltipProvider>
