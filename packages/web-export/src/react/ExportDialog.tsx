// React copy of ExportDialog.svelte, on Base UI's Dialog. The Export dialog (D9, the ExportFrame board): the shell of it
// (the scrim, the focus, Escape, the title and the close button) around `ExportBody`. The HTML, the report and the limit
// are the app layer's (`state.export.dialog`, built while the dialog is open); it keeps view state only (`ui.ts`: the
// tab, the file shown, what the last Copy said), which starts again for every dialog. Its PNG is of the dialog's own
// frame. PDF is not offered (the epics defer it). A text with a value in it is one string (`{`Export ${name}`}`), as the
// Svelte copy draws it: React makes two text nodes of `Export {name}`, the browser shapes them as two runs, and the clip
// of the truncated title then ended a fraction of a pixel elsewhere (its last letter differed in some pixels).
import { Dialog } from '@base-ui/react/dialog';
import type { Action, ExportDialog as Model } from '@meldframe/protocol';
import { IconButton } from '@meldframe/web-chrome/react/IconButton';
import { TooltipProvider } from '@meldframe/web-chrome/react/Tooltip';
import { useFocusHandoff } from '@meldframe/web-chrome/react/useFocusHandoff';
import type { ExportScale } from '@meldframe/web-document/export';
import { nameOf } from '@meldframe/web-document/names';
import { useRef, useState } from 'react';
import type { PngPlan } from '../png';
import { START, type View, viewFor } from '../ui';
import { ExportBody } from './ExportBody';

export interface ExportDialogProps {
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

export function ExportDialog({ dialog, unreviewed, scale, onScale, plan, onPng, dispatch, onReview }: ExportDialogProps) {
  const [held, setHeld] = useState<{ frame: string | null; view: View }>({ frame: null, view: START });
  // A dialog that opened, closed or moved to another frame starts again: the view is derived while drawing (React's
  // pattern for state that follows props), so the first frame of a new dialog already shows the start view.
  const ui = viewFor(held, dialog?.frame ?? null);
  if (ui !== held) setHeld(ui);
  const tab = useRef(ui.view.tab);
  tab.current = ui.view.tab;

  // A dialog opened from a menu gives the focus back to the menu's button when it closes (the handoff took it when this opened); any other close takes the default (the element that had the focus).
  const hand = useFocusHandoff(dialog !== null);
  const close = () => dispatch({ type: 'closeExport' });
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
            data-testid="export-dialog"
            // The first thing to focus is the selected tab, not the Close button: a tooltip opens on that button and takes the first Escape.
            initialFocus={() => document.getElementById(`export-tab-${tab.current}`)}
            finalFocus={hand.finalFocus}
            // Bits UI sets this on its content, and it moves how a box at a half pixel is snapped to the device grid (the count badge of the
            // report sat 1 device pixel lower without it, at a scale factor of 2): kept, so that the two dialogs paint the same pixels.
            style={{ contain: 'layout style' }}
            className="fixed inset-8 z-50 m-auto flex h-fit max-h-full w-full max-w-3xl flex-col overflow-hidden rounded-lg border border-divider bg-surface font-sans text-body text-text shadow-dialog"
          >
            {dialog ? (
              <>
                <header className="flex h-14 shrink-0 items-center justify-between border-b border-hairline pr-3 pl-5">
                  <Dialog.Title className="m-0 truncate text-title font-semibold text-text-strong">{`Export ${nameOf(dialog.name)}`}</Dialog.Title>
                  <Dialog.Close render={<IconButton label="Close" icon="close" />} />
                </header>
                <Dialog.Description className="sr-only">{`Export the frame ${nameOf(dialog.name)} as HTML or as a PNG image. Your file is not changed.`}</Dialog.Description>
                <ExportBody dialog={dialog} unreviewed={unreviewed} view={ui.view} onView={(view) => setHeld((h) => ({ ...h, view }))} scale={scale} onScale={onScale} plan={plan} onPng={onPng} dispatch={dispatch} onReview={onReview} onClose={close} />
              </>
            ) : null}
          </Dialog.Popup>
        </Dialog.Portal>
      </Dialog.Root>
    </TooltipProvider>
  );
}
