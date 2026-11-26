<!--
  Development only, part of the React pilot (docs/react-web-plan.md, section 7): the Svelte reference of FixtureDialog.tsx,
  the dialog that the "Dialog…" row of the overlay fixture opens (/?overlays=svelte). A small Bits UI Dialog that gives
  the focus back as ExportDialog.svelte does (`takeFocusBack` on `onCloseAutoFocus`), so that overlays-e2e.mjs follows
  the focus from the menu button into it and back on both pages.
-->
<script lang="ts">
  import { Dialog } from 'bits-ui';
  import { takeFocusBack } from '@meldframe/web-chrome/focus';

  let { open, onClose }: { open: boolean; onClose: () => void } = $props();
  const giveBack = (e: Event) => {
    const el = takeFocusBack();
    if (el) {
      e.preventDefault();
      el.focus();
    }
  };
</script>

<Dialog.Root {open} onOpenChange={(next) => { if (!next) onClose(); }}>
  <Dialog.Portal>
    <Dialog.Overlay class="fixed inset-0 z-40 bg-scrim" />
    <Dialog.Content data-owns-keys data-testid="fx-dialog-box" onCloseAutoFocus={giveBack} class="fixed top-24 left-1/2 z-50 flex w-full max-w-sm -translate-x-1/2 flex-col gap-3 rounded-lg border border-divider bg-surface p-4 font-sans text-body text-text shadow-dialog">
      <Dialog.Title class="m-0 text-title font-semibold text-text-strong">Fixture dialog</Dialog.Title>
      <Dialog.Description class="sr-only">A stand-in that takes the focus when it opens.</Dialog.Description>
      <input data-testid="fx-dialog-input" aria-label="Dialog stand-in" class="h-(--mf-size-control-lg) rounded-md border border-divider bg-field px-3 text-small" />
      <div class="flex gap-2">
        <Dialog.Close data-testid="fx-dialog-close" class="h-(--mf-size-control-lg) rounded-md bg-control px-4 text-small">Close</Dialog.Close>
      </div>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
