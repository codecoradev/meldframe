<!--
  The Export dialog (D9, the ExportFrame board): the shell of it (Bits UI: the scrim, the focus, Escape, the title and the
  close button) around `ExportBody`. The HTML, the report and the limit are the app layer's (`state.export.dialog`, built
  while the dialog is open); it keeps view state only (`ui.ts`: the tab, the file shown, what the last Copy said), which
  starts again for every dialog. Its PNG is of the dialog's own frame. PDF is not offered (the epics defer it).
-->
<script lang="ts">
  import { Dialog } from 'bits-ui';
  import type { Action, ExportDialog as Model } from '@meldframe/protocol';
  import IconButton from '@meldframe/web-chrome/IconButton.svelte';
  import TooltipProvider from '@meldframe/web-chrome/TooltipProvider.svelte';
  import { takeFocusBack } from '@meldframe/web-chrome/focus';
  import type { ExportScale } from '@meldframe/web-document/export';
  import ExportBody from './ExportBody.svelte';
  import type { PngPlan } from './png';
  import { START, viewFor, type View } from './ui';
  import { plain } from './view';

  interface Props {
    /** `state.export.dialog`; null while closed. */
    dialog: Model | null;
    /** `state.activity.unreviewedCount`. */
    unreviewed: number | null;
    scale: ExportScale;
    onScale: (s: ExportScale) => void;
    /** The PNG of the dialog's frame (`dialogFrame`) as the canvas would draw it now. */
    plan: PngPlan | null;
    onPng: () => void;
    dispatch: (a: Action) => void;
    /** The dialog closes and the page opens the Activity tab. */
    onReview: () => void;
  }
  let { dialog, unreviewed, scale, onScale, plan, onPng, dispatch, onReview }: Props = $props();

  let ui = $state<{ frame: string | null; view: View }>({ frame: null, view: START });
  $effect.pre(() => {
    const next = viewFor(ui, dialog?.frame ?? null);
    if (next !== ui) ui = next;
  });
  const close = () => dispatch({ type: 'closeExport' });
  // The first thing to focus is the selected tab, not the Close button: a tooltip opens on that button and takes the first Escape.
  const focusTab = (e: Event) => {
    e.preventDefault();
    document.getElementById(`export-tab-${ui.view.tab}`)?.focus();
  };
  // A dialog opened from a menu gives the focus back to the menu's button when it closes.
  const giveBack = (e: Event) => {
    const el = takeFocusBack();
    if (el) {
      e.preventDefault();
      el.focus();
    }
  };
</script>

<TooltipProvider>
<Dialog.Root open={dialog !== null} onOpenChange={(o) => { if (!o) close(); }}>
  <Dialog.Portal>
    <Dialog.Overlay class="fixed inset-0 z-40 bg-scrim" />
    <Dialog.Content data-owns-keys data-testid="export-dialog" onOpenAutoFocus={focusTab} onCloseAutoFocus={giveBack} class="fixed inset-8 z-50 m-auto flex h-fit max-h-full w-full max-w-3xl flex-col overflow-hidden rounded-lg border border-divider bg-surface font-sans text-body text-text shadow-dialog">
      {#if dialog}
        <header class="flex h-14 shrink-0 items-center justify-between border-b border-hairline pr-3 pl-5">
          <Dialog.Title class="m-0 truncate text-title font-semibold text-text-strong">Export {plain(dialog.name)}</Dialog.Title>
          <Dialog.Close>{#snippet child({ props })}<IconButton label="Close" icon="close" triggerProps={props} />{/snippet}</Dialog.Close>
        </header>
        <Dialog.Description class="sr-only">Export the frame {plain(dialog.name)} as HTML or as a PNG image. Your file is not changed.</Dialog.Description>
        <ExportBody {dialog} {unreviewed} view={ui.view} onView={(v) => (ui = { ...ui, view: v })} {scale} {onScale} {plan} {onPng} {dispatch} {onReview} onClose={close} />
      {/if}
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
</TooltipProvider>
