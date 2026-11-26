// Development only, part of the React pilot (docs/react-web-plan.md, section 7): the dialog that the "Dialog…" row of the
// overlay fixture opens (/?overlays=react). A small Base UI Dialog on the focus handoff, as the Export dialog is, so that
// packages/web-chrome/scripts/overlays-e2e.mjs follows the focus from the menu button into a real dialog and back (the Svelte
// side has FixtureDialogSvelte.svelte). It has one thing the Svelte one has not: a button that draws the popup as another
// element (a section for a div) while the dialog is open. The focus manager stays, its dependency on the popup element
// changes, and so its cleanup runs and asks `finalFocus` in the middle of an open: the second call the handoff must survive.
import { Dialog } from '@base-ui/react/dialog';
import { useFocusHandoff } from '@meldframe/web-chrome/react/useFocusHandoff';
import { useEffect, useRef, useState } from 'react';

export function FixtureDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const hand = useFocusHandoff(open);
  const [drawn, setDrawn] = useState(0);
  const field = useRef<HTMLInputElement>(null);
  // What it will give the focus to, shown so that overlays-e2e.mjs can read the handoff itself and not only where the focus ended up
  // (the library's own return often lands on the same element): the test id of the element, or `default` when it was handed none.
  const [back, setBack] = useState('');
  useEffect(() => {
    const to = hand.finalFocus();
    setBack(open ? (to ? (to.getAttribute('data-testid') ?? to.tagName) : 'default') : '');
  }, [open, hand]);
  return (
    <Dialog.Root
      open={open}
      onOpenChange={(next) => {
        if (!next) onClose();
      }}
    >
      <Dialog.Portal>
        <Dialog.Backdrop className="fixed inset-0 z-40 bg-scrim" />
        <Dialog.Popup
          render={(props) => (drawn % 2 ? <section {...props} /> : <div {...props} />)}
          data-owns-keys=""
          data-testid="fx-dialog-box"
          data-drawn={drawn}
          data-return={back}
          initialFocus={() => field.current}
          finalFocus={hand.finalFocus}
          className="fixed top-24 left-1/2 z-50 flex w-full max-w-sm -translate-x-1/2 flex-col gap-3 rounded-lg border border-divider bg-surface p-4 font-sans text-body text-text shadow-dialog"
        >
          <Dialog.Title className="m-0 text-title font-semibold text-text-strong">Fixture dialog</Dialog.Title>
          <Dialog.Description className="sr-only">A stand-in that takes the focus when it opens.</Dialog.Description>
          <input ref={field} data-testid="fx-dialog-input" aria-label="Dialog stand-in" className="h-(--mf-size-control-lg) rounded-md border border-divider bg-field px-3 text-small" />
          <div className="flex gap-2">
            <button type="button" data-testid="fx-dialog-redraw" onClick={() => setDrawn((n) => n + 1)} className="h-(--mf-size-control-lg) rounded-md bg-control px-4 text-small">
              Draw again
            </button>
            <Dialog.Close data-testid="fx-dialog-close" className="h-(--mf-size-control-lg) rounded-md bg-control px-4 text-small">
              Close
            </Dialog.Close>
          </div>
        </Dialog.Popup>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
