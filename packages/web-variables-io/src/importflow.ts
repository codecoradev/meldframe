// What the Import variables dialog holds and does, with no framework: the text of the field and the note about the file (`flow.ts` has the
// steps), and what each thing the person does comes to, in what order, and what is sent to the app layer. The React dialog reads it
// through `useSyncExternalStore`; the Svelte dialog keeps the same few lines inline. The result and its words are the app layer's
// (`state.variablesIo.import`, `io.ts`); this keeps view state only, for the life of the page: closing the dialog keeps the text, and an
// import that was applied, or Clear, starts it again.
import type { Action, VariablesImportResult } from '@meldframe/protocol';
import { FRESH, type ImportView, applied, edited, fileInto, latest, stale } from './flow';
import { sizeText } from './io';
import type { Read } from './read';

export interface ImportFlowDeps<F extends { name: string; size: number }> {
  dispatch: (a: Action) => void;
  /** The result of the app layer now, read when a step needs it (the answer to a click arrives after the click). */
  result: () => VariablesImportResult | null;
  /** Reads a chosen file (`read.ts`; a fake in a test). */
  read: (file: F) => Promise<Read>;
}

// Closing the dialog closes it and forgets the result; a text that changes forgets the result and the name of the file and leaves the dialog open.
const CLOSE: Action = { type: 'closeVariablesImport' };
const CLEAR: Action = { type: 'clearVariablesImport' };

export function createImportFlow<F extends { name: string; size: number }>(deps: ImportFlowDeps<F>) {
  let view: ImportView = FRESH;
  const guard = latest();
  const listeners = new Set<() => void>();
  const set = (next: ImportView) => {
    view = next;
    for (const l of [...listeners]) l();
  };
  /** The result belongs to the text it was made from, and so does the name of the file the text came from: a text that changes puts both away. */
  const putAway = (before: ImportView) => {
    if (stale(deps.result(), before)) deps.dispatch(CLEAR);
  };
  return {
    view: (): ImportView => view,
    /** Calls `fn` after each change of the view; returns the function that stops it. */
    subscribe(fn: () => void): () => void {
      listeners.add(fn);
      return () => void listeners.delete(fn);
    },
    /** The person typed. What is typed also ends a file that is still being read, so the file's text never replaces it. */
    edit(text: string) {
      const before = view;
      set(edited(guard, view, text));
      putAway(before);
    },
    /** Clear: the field and the note start again, and a file under way goes stale. */
    clear() {
      const before = view;
      guard.cancel();
      set(FRESH);
      putAway(before);
    },
    /** A file was chosen. It fills the field when it is read and still the latest; a file that is not read leaves the field as it was, with the reason. */
    async choose(file: F) {
      const next = await fileInto(guard, file, () => view, deps.read, sizeText);
      if (!next) return;
      // The result of an earlier text is not about this one. The app layer makes the name of the file safe to show: only the name of a file that was read goes.
      if (deps.result()) deps.dispatch(CLEAR);
      if (next.note === null) deps.dispatch({ type: 'chooseVariablesImportSource', name: file.name });
      set(next);
    },
    /** The dialog closed (Escape, Close, the scrim): the app layer forgets the result, a file under way must not fill a field the person has left, and the text stays. */
    close() {
      guard.cancel();
      deps.dispatch(CLOSE);
    },
    /**
     * An import was applied: the field and the note start again (a second press would only be refused as a duplicate). True when that changed
     * what is drawn; false when the field and the note were empty already (Clear was pressed in the gap), and then nothing is told to a listener.
     */
    imported(): boolean {
      if (view.text === '' && view.note === null) return false;
      set(applied());
      return true;
    },
  };
}
