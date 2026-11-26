<!--
  The Export variables dialog (D8): every variable of the file as CSS custom properties, one per line, in a preview with
  Copy and Download. The text, its counts and what it could not write as stored are the app layer's
  (`state.variablesIo.export`, built while the dialog is open); this draws them in a dialog (Bits UI: the scrim, the focus,
  Escape, the title and the close button) and keeps one bit of view state, what the last Copy said. Open or closed is the
  app state: Close and Escape send `closeVariablesExport`. The focus goes into the dialog and back to the button that opened it.
-->
<script lang="ts">
  import { Dialog } from 'bits-ui';
  import type { Action, VariablesExportDialog as Model } from '@meldframe/protocol';
  import IconButton from '@meldframe/web-chrome/IconButton.svelte';
  import TooltipProvider from '@meldframe/web-chrome/TooltipProvider.svelte';
  import ExportVariablesBody from './ExportVariablesBody.svelte';
  import { type Said, saidFor } from './flow';
  import { copyExport, saveExport } from './host';

  interface Props {
    /** `state.variablesIo.export`; null while closed. */
    dialog: Model | null;
    dispatch: (a: Action) => void;
    /** The button that opened the dialog, to give the focus back to. */
    opener: () => HTMLElement | null | undefined;
  }
  let { dialog, dispatch, opener }: Props = $props();

  let said = $state<Said | null>(null);
  $effect(() => {
    if (!dialog) said = null;
  });
  const close = () => dispatch({ type: 'closeVariablesExport' });
  async function copy() {
    if (!dialog) return;
    said = await copyExport(dialog.css);
  }
  // The first thing to focus is Copy (the preview when there is nothing to copy), not the Close icon: a tooltip opens on that button and takes the first Escape.
  const focusStart = (e: Event) => {
    const el = document.querySelector<HTMLElement>('[data-testid=export-vars-copy]:not(:disabled)') ?? document.querySelector<HTMLElement>('[data-testid=export-vars-dialog] [role=region]') ?? document.querySelector<HTMLElement>('[data-testid=export-vars-done]');
    if (el) {
      e.preventDefault();
      el.focus();
    }
  };
  const giveBack = (e: Event) => {
    const el = opener();
    if (el?.isConnected) {
      e.preventDefault();
      el.focus();
    }
  };
</script>

<TooltipProvider>
  <Dialog.Root open={dialog !== null} onOpenChange={(o) => { if (!o) close(); }}>
    <Dialog.Portal>
      <Dialog.Overlay class="fixed inset-0 z-40 bg-scrim" />
      <Dialog.Content data-owns-keys data-testid="export-vars-dialog" onOpenAutoFocus={focusStart} onCloseAutoFocus={giveBack} class="fixed inset-8 z-50 m-auto flex h-fit max-h-full w-full max-w-3xl flex-col overflow-hidden rounded-lg border border-divider bg-surface font-sans text-body text-text shadow-dialog">
        <header class="flex h-14 shrink-0 items-center justify-between border-b border-hairline pr-3 pl-5">
          <Dialog.Title class="m-0 truncate text-title font-semibold text-text-strong">Export variables</Dialog.Title>
          <Dialog.Close>{#snippet child({ props })}<IconButton label="Close" icon="close" triggerProps={props} data-testid="export-vars-close" />{/snippet}</Dialog.Close>
        </header>
        <Dialog.Description class="sr-only">Every variable of this file as CSS custom properties, to copy or save. Your file is not changed.</Dialog.Description>
        {#if dialog}
          <ExportVariablesBody {dialog} said={saidFor(said, dialog.css)} onCopy={copy} onDownload={() => saveExport(dialog.css)} onClose={close} />
        {/if}
      </Dialog.Content>
    </Dialog.Portal>
  </Dialog.Root>
</TooltipProvider>
