// React copy of Palette.svelte, on Base UI's Dialog. The command palette (D10): the shell of it (the scrim, the focus trap,
// Escape, the name) around `PaletteBody`. Ctrl+K or Command+K opens it; the focus goes to the search field and, when it
// closes, back to where it was. Running an entry closes it and calls the entry's handler. An entry that opens a dialog of
// its own (Export, the shortcut sheet) takes the focus itself and gets the place to give it back to, as a menu row does
// (`pass` of the focus handoff): such an entry waits until the close is complete, then runs, so the palette has let go of
// the focus by then. Neither `initialFocus` nor `finalFocus` has a side effect: Base UI may call them more than once.
import { Dialog } from '@base-ui/react/dialog';
import { useFocusHandoff } from '@meldframe/web-chrome/react/useFocusHandoff';
import { useRef } from 'react';
import type { PaletteEntry } from '../palette';
import { PaletteBody } from './PaletteBody';

export interface PaletteProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  entries: PaletteEntry[];
}

export function Palette({ open, onOpenChange, entries }: PaletteProps) {
  const hand = useFocusHandoff(open);
  const input = useRef<HTMLInputElement>(null);
  // An entry that opens a dialog waits until the palette has closed and let go of the focus, or the focus would be
  // taken from the dialog as the palette goes; the others run at once and the focus goes back to where it was.
  const run = (e: PaletteEntry) => {
    if (e.opensDialog) hand.pass(hand.opener, e.run);
    onOpenChange(false);
    if (!e.opensDialog) e.run();
  };
  return (
    <Dialog.Root
      open={open}
      onOpenChange={(next) => onOpenChange(next)}
      onOpenChangeComplete={(done) => {
        if (!done) hand.closed();
      }}
    >
      <Dialog.Portal>
        <Dialog.Backdrop className="fixed inset-0 z-40 bg-scrim" />
        <Dialog.Popup
          data-owns-keys=""
          data-testid="palette"
          // The palette is a modal dialog: Bits UI says so in `aria-modal`, Base UI hides the page behind from readers instead and leaves the attribute off.
          aria-modal="true"
          // The field takes the first focus; where the focus was before is what it goes back to (the library's default), unless an
          // entry opens a dialog, which keeps the focus away from the page.
          initialFocus={() => input.current}
          finalFocus={hand.finalFocus}
          // Bits UI sets this on its content, and it moves how a box at a half pixel is snapped to the device grid: kept, so that the two palettes paint the same pixels.
          style={{ contain: 'layout style' }}
          className="fixed top-24 left-1/2 z-50 w-full max-w-xl -translate-x-1/2 overflow-hidden rounded-lg border border-menu-border bg-surface-raised font-sans text-body text-text shadow-popover"
        >
          <Dialog.Title className="sr-only">Command palette</Dialog.Title>
          <Dialog.Description className="sr-only">Type to narrow the list. The up and down arrows choose, Enter runs, Escape closes.</Dialog.Description>
          <PaletteBody entries={entries} onRun={run} onClose={() => onOpenChange(false)} inputRef={input} />
        </Dialog.Popup>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
