<!--
  The command palette (D10): the shell of it (Bits UI: the scrim, the focus trap, Escape, the name) around `PaletteBody`.
  Ctrl+K or Command+K opens it; the focus goes to the search field and, when it closes, back to where it was. Running an
  entry closes it and calls the entry's handler. An entry that opens a dialog of its own (Export, the shortcut sheet) takes
  the focus itself and gets the place to give it back to, as a menu row does (`rememberFocus`).
-->
<script lang="ts">
  import { Dialog } from 'bits-ui';
  import { rememberFocus } from '@meldframe/web-chrome/focus';
  import PaletteBody from './PaletteBody.svelte';
  import type { PaletteEntry } from './palette';

  interface Props {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    entries: PaletteEntry[];
  }
  let { open, onOpenChange, entries }: Props = $props();

  let before: HTMLElement | null = null;
  let keepFocus = false;
  let later: (() => void) | null = null;
  // An entry that opens a dialog waits until the palette has closed and let go of the focus, or the focus would be
  // taken from the dialog as the palette goes; the others run at once and the focus goes back to where it was.
  const run = (e: PaletteEntry) => {
    keepFocus = !!e.opensDialog;
    if (keepFocus) {
      rememberFocus(before);
      later = e.run;
    }
    onOpenChange(false);
    if (!keepFocus) e.run();
  };
  const closed = (ev: Event) => {
    if (keepFocus) ev.preventDefault();
    keepFocus = false;
    const f = later;
    later = null;
    f?.();
  };
</script>

<Dialog.Root {open} {onOpenChange}>
  <Dialog.Portal>
    <Dialog.Overlay class="fixed inset-0 z-40 bg-scrim" />
    <Dialog.Content
      data-owns-keys
      data-testid="palette"
      onOpenAutoFocus={() => (before = document.activeElement as HTMLElement | null)}
      onCloseAutoFocus={closed}
      class="fixed top-24 left-1/2 z-50 w-full max-w-xl -translate-x-1/2 overflow-hidden rounded-lg border border-menu-border bg-surface-raised font-sans text-body text-text shadow-popover"
    >
      <Dialog.Title class="sr-only">Command palette</Dialog.Title>
      <Dialog.Description class="sr-only">Type to narrow the list. The up and down arrows choose, Enter runs, Escape closes.</Dialog.Description>
      <PaletteBody {entries} onRun={run} onClose={() => onOpenChange(false)} />
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
