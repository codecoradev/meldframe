// React copy of ManageDialog.svelte, on Base UI's Dialog. The manage view in a dialog (the scrim, the focus, Escape, the title and the
// close button) around `ManageBody`. It is opened from the Variables panel and gives the focus back to the button that opened it. Open
// or closed is the page's view state (the caller owns it); the content is the app state. Whatever closes it (the Close button, the
// scrim, Escape) tells the app layer (`ManageOp::Close`), which forgets the question, the rename field, the forms and the choice of a
// collection; both copies of the Variables panel draw this dialog through the switch, so this is the one place.
import { Dialog } from '@base-ui/react/dialog';
import type { Action, Event, ManageState, VariablesPanel } from '@meldframe/protocol';
import { IconButton } from '@meldframe/web-chrome/react/IconButton';
import { TooltipProvider } from '@meldframe/web-chrome/react/Tooltip';
import { useFocusHandoff } from '@meldframe/web-chrome/react/useFocusHandoff';
import { ManageBody } from './ManageBody';

export interface ManageDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  panel: VariablesPanel;
  /** `AppState.manage`: what the manage view shows and asks. */
  manage: ManageState;
  events: readonly Event[];
  dispatch: (a: Action) => void;
  /** The button that opened the dialog, to give the focus back to. */
  opener: () => HTMLElement | null | undefined;
}

// The first thing to focus is the collection on view, not the Close button: a tooltip opens on that button and takes the first Escape.
// Null (nothing to focus yet) is Base UI's own first focus, as it is Bits UI's.
const focusStart = () => document.querySelector<HTMLElement>('[data-testid=manage-dialog] [aria-current=true]') ?? document.querySelector<HTMLElement>('[data-testid=manage-new-collection]');

export function ManageDialog({ open, onOpenChange, panel, manage, events, dispatch, opener }: ManageDialogProps) {
  // The button that opened the dialog gets the focus back, as in the Svelte copy; when the caller cannot name it (or it left the page), the
  // handoff's answer stands: what an earlier overlay left for this one, else the element that had the focus when this opened.
  const hand = useFocusHandoff(open);
  return (
    <TooltipProvider>
      <Dialog.Root
        open={open}
        onOpenChange={(next) => {
          if (!next) dispatch({ type: 'manage', op: { type: 'close' } });
          onOpenChange(next);
        }}
      >
        <Dialog.Portal>
          <Dialog.Backdrop className="fixed inset-0 z-40 bg-scrim" />
          <Dialog.Popup
            data-owns-keys=""
            data-testid="manage-dialog"
            initialFocus={focusStart}
            finalFocus={() => {
              const el = opener();
              return el?.isConnected ? el : hand.finalFocus();
            }}
            // Bits UI sets this on its content, and it moves how a box at a half pixel is snapped to the device grid: kept, so that the two dialogs paint the same pixels (S2).
            style={{ contain: 'layout style' }}
            className="fixed inset-8 z-50 m-auto flex h-fit max-h-full min-h-96 w-full max-w-4xl flex-col overflow-hidden rounded-lg border border-divider bg-surface font-sans text-body text-text shadow-dialog"
          >
            <header className="flex h-14 shrink-0 items-center justify-between border-b border-hairline pr-3 pl-5">
              <Dialog.Title className="m-0 truncate text-title font-semibold text-text-strong">Manage variables</Dialog.Title>
              <Dialog.Close render={<IconButton label="Close" icon="close" data-testid="manage-close" />} />
            </header>
            <Dialog.Description className="sr-only">Add and remove collections, modes and variables. A removal asks first, and one undo takes it back.</Dialog.Description>
            <ManageBody panel={panel} manage={manage} events={events} dispatch={dispatch} />
          </Dialog.Popup>
        </Dialog.Portal>
      </Dialog.Root>
    </TooltipProvider>
  );
}
