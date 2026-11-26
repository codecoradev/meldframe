// React copy of ExportVariablesDialog.svelte, on Base UI's Dialog. The Export variables dialog (D8): every variable of the file as CSS custom
// properties, one per line, in a preview with Copy and Download. The text, its counts and what it could not write as stored are the app layer's
// (`state.variablesIo.export`, built while the dialog is open); this draws them in a dialog (the scrim, the focus, Escape, the title and the
// close button) and keeps one bit of view state, what the last Copy said. Open or closed is the app state: Close and Escape send
// `closeVariablesExport`. The focus goes into the dialog (Copy, the preview when there is nothing to copy) and back to the button that opened it.
// The clipboard and the file save are `host.ts`, shared with the Svelte copy.
import { Dialog } from '@base-ui/react/dialog';
import type { Action, VariablesExportDialog as Model } from '@meldframe/protocol';
import { IconButton } from '@meldframe/web-chrome/react/IconButton';
import { TooltipProvider } from '@meldframe/web-chrome/react/Tooltip';
import { useFocusHandoff } from '@meldframe/web-chrome/react/useFocusHandoff';
import { useState } from 'react';
import { type Said, saidFor } from '../flow';
import { copyExport, saveExport } from '../host';
import { ExportBody } from './ExportBody';

export interface ExportDialogProps {
  /** `state.variablesIo.export`; null while closed. */
  dialog: Model | null;
  dispatch: (a: Action) => void;
  /** The button that opened the dialog, to give the focus back to. */
  opener: () => HTMLElement | null | undefined;
}

// The first thing to focus is Copy (the preview when there is nothing to copy), not the Close icon: a tooltip opens on that button and takes the first Escape.
const focusStart = () =>
  document.querySelector<HTMLElement>('[data-testid=export-vars-copy]:not(:disabled)') ??
  document.querySelector<HTMLElement>('[data-testid=export-vars-dialog] [role=region]') ??
  document.querySelector<HTMLElement>('[data-testid=export-vars-done]');

export function ExportDialog({ dialog, dispatch, opener }: ExportDialogProps) {
  const [said, setSaid] = useState<Said | null>(null);
  // A dialog that closed starts again, with nothing said: derived while drawing (React's pattern for state that follows props).
  if (dialog === null && said !== null) setSaid(null);
  // The button that opened the dialog gets the focus back; when it left the page, the handoff's answer stands.
  const hand = useFocusHandoff(dialog !== null);
  const close = () => dispatch({ type: 'closeVariablesExport' });
  return (
    <TooltipProvider>
      <Dialog.Root
        open={dialog !== null}
        onOpenChange={(open) => {
          if (!open) close();
        }}
      >
        <Dialog.Portal>
          <Dialog.Backdrop className="fixed inset-0 z-40 bg-scrim" />
          <Dialog.Popup
            data-owns-keys=""
            data-testid="export-vars-dialog"
            initialFocus={focusStart}
            finalFocus={() => {
              const el = opener();
              return el?.isConnected ? el : hand.finalFocus();
            }}
            // Bits UI sets this on its content, and it moves how a box at a half pixel is snapped to the device grid: kept, so that the two dialogs paint the same pixels (S2).
            style={{ contain: 'layout style' }}
            className="fixed inset-8 z-50 m-auto flex h-fit max-h-full w-full max-w-3xl flex-col overflow-hidden rounded-lg border border-divider bg-surface font-sans text-body text-text shadow-dialog"
          >
            <header className="flex h-14 shrink-0 items-center justify-between border-b border-hairline pr-3 pl-5">
              <Dialog.Title className="m-0 truncate text-title font-semibold text-text-strong">Export variables</Dialog.Title>
              <Dialog.Close render={<IconButton label="Close" icon="close" data-testid="export-vars-close" />} />
            </header>
            <Dialog.Description className="sr-only">Every variable of this file as CSS custom properties, to copy or save. Your file is not changed.</Dialog.Description>
            {dialog ? (
              <ExportBody
                dialog={dialog}
                said={saidFor(said, dialog.css)}
                onCopy={async () => setSaid(await copyExport(dialog.css))}
                onDownload={() => saveExport(dialog.css)}
                onClose={close}
              />
            ) : null}
          </Dialog.Popup>
        </Dialog.Portal>
      </Dialog.Root>
    </TooltipProvider>
  );
}
