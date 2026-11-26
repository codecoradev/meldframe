// React copy of ImportVariablesDialog.svelte, on Base UI's Dialog. The Import variables dialog (D8): CSS text pasted into a field or read from
// a file the person chooses (in the browser, nothing is uploaded), sent to the app layer as `importVariables`, and the result drawn under it:
// what was added, what was skipped and why, or why nothing was imported. The app layer reads the text, checks the commands, and applies them as
// one undo step of the person's own, or applies nothing (`state.variablesIo.import`); this holds the text of the field and the note about the
// file (`importflow.ts`, no framework), and words the result (`io.ts`). Open or closed is the page's view state (the caller owns it); closing
// sends `closeVariablesImport` and keeps the text. The focus goes into the field and back to the button that opened the dialog.
import { Dialog } from '@base-ui/react/dialog';
import type { Action, VariablesImportResult } from '@meldframe/protocol';
import { IconButton } from '@meldframe/web-chrome/react/IconButton';
import { TooltipProvider } from '@meldframe/web-chrome/react/Tooltip';
import { useFocusHandoff } from '@meldframe/web-chrome/react/useFocusHandoff';
import { useEffect, useLayoutEffect, useRef, useState, useSyncExternalStore } from 'react';
import { stranded } from '../flow';
import { createImportFlow } from '../importflow';
import { readFile } from '../read';
import { ImportBody } from './ImportBody';

export interface ImportDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** `state.variablesIo.import`. */
  result: VariablesImportResult | null;
  dispatch: (a: Action) => void;
  /** The button that opened the dialog, to give the focus back to. */
  opener: () => HTMLElement | null | undefined;
}

// The first thing to focus is the field, not the Close button: a tooltip opens on that button and takes the first Escape.
const focusStart = () => document.querySelector<HTMLElement>('[data-testid=import-vars-text]');

export function ImportDialog({ open, onOpenChange, result, dispatch, opener }: ImportDialogProps) {
  const hand = useFocusHandoff(open);
  // What the flow reads when a step needs it: the answer of the app layer arrives after the click that asked.
  const now = useRef({ result, dispatch });
  useLayoutEffect(() => {
    now.current = { result, dispatch };
  });
  // What the field holds, kept for the life of the page (this component outlives the dialog): closing keeps the text, and an import that was applied, or Clear, starts it again.
  const [flow] = useState(() => createImportFlow<File>({ dispatch: (a) => now.current.dispatch(a), result: () => now.current.result, read: readFile }));
  // The same stored value on the server: the component draws in Node (its test) as well as in the browser.
  const view = useSyncExternalStore(flow.subscribe, flow.view, flow.view);

  // A primitive, so a new state of the app layer that carries the same result does not empty what the person has typed since.
  const done = result?.status === 'imported';
  const refocus = useRef(false);
  // The field empties after an import, so Import and Clear become disabled; a focus left on one of them would leave the keyboard with nothing to
  // act on inside the modal. It goes to Close (the next thing to do), unless it is on a control that works.
  const keepFocus = () => {
    if (stranded(document.activeElement as HTMLButtonElement | null, document.body)) document.querySelector<HTMLElement>('[data-testid=import-vars-done]')?.focus();
  };
  useEffect(() => {
    if (!done) return;
    // The emptied field is drawn by the layout effect below. When there is nothing to empty (Clear was pressed in the gap) no draw follows, so the
    // flag must not be left set for the next change: the check runs here, on the field as it is.
    if (flow.imported()) refocus.current = true;
    else keepFocus();
  }, [done, flow]);
  // Once the emptied field is drawn.
  useLayoutEffect(() => {
    if (!refocus.current || view.text !== '') return;
    refocus.current = false;
    keepFocus();
  }, [view]);

  const change = (next: boolean) => {
    if (!next) flow.close();
    onOpenChange(next);
  };
  return (
    <TooltipProvider>
      <Dialog.Root open={open} onOpenChange={change}>
        <Dialog.Portal>
          <Dialog.Backdrop className="fixed inset-0 z-40 bg-scrim" />
          <Dialog.Popup
            data-owns-keys=""
            data-testid="import-vars-dialog"
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
              <Dialog.Title className="m-0 truncate text-title font-semibold text-text-strong">Import variables</Dialog.Title>
              <Dialog.Close render={<IconButton label="Close" icon="close" data-testid="import-vars-close" />} />
            </header>
            <Dialog.Description className="sr-only">Add the variables of CSS text to this file. Paste the text or choose a file; one undo takes the import back.</Dialog.Description>
            <ImportBody
              result={result}
              text={view.text}
              onText={flow.edit}
              note={view.note}
              onFile={(file) => void flow.choose(file)}
              onClear={flow.clear}
              onImport={() => dispatch({ type: 'importVariables', text: flow.view().text })}
              onClose={() => change(false)}
            />
          </Dialog.Popup>
        </Dialog.Portal>
      </Dialog.Root>
    </TooltipProvider>
  );
}
