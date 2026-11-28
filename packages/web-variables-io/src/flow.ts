// The view state of the Export variables and Import variables dialogs, as pure steps. What each dialog shows is the app
// layer's (`state.variablesIo`); this is what the page keeps on top of it, and the rules for when that is put away.
import type { Action } from '@meldframe/protocol';
import { nameOf } from '@meldframe/web-document/names';

/** What the last Copy said, and the text it was about. */
export interface Said {
  ok: boolean;
  text: string;
  /** The text that was copied (or that the browser refused to copy). */
  css: string;
}

/** What the last Copy said, while it is still about the text on view: a text that changed under an open dialog (another actor added a variable) makes it stale. */
export function saidFor(said: Said | null, css: string): { ok: boolean; text: string } | null {
  return said !== null && said.css === css ? { ok: said.ok, text: said.text } : null;
}

/** What the person was told about the file they chose: read, or why not. */
export interface Note {
  ok: boolean;
  text: string;
}

/**
 * What the Import field holds: the text, the words for the size of the file it came from (the name of that file is the app
 * layer's, `state.variablesIo.importSource`: it makes it safe to show, rule U9), and the reason a file was not read.
 */
export interface ImportView {
  text: string;
  note: Note | null;
  size: string | null;
}

export const FRESH: ImportView = { text: '', note: null, size: null };

/** The person typed: the text changes, and the note about a file no longer describes it. */
export const typed = (_: ImportView, text: string): ImportView => ({ text, note: null, size: null });

/** The note under the field: why a file was not read, or that one was, with the name as the app layer gave it. */
export const noteOf = (v: ImportView, file: string | null | undefined): Note | null =>
  v.note ?? (v.size !== null && typeof file === 'string' ? { ok: true, text: `Read ${nameOf(file)} (${v.size}). Check it, then import.` } : null);

/** Whether the app layer still holds something about the field that goes when the text changes: a result, or the name of the file. */
export const stale = (result: unknown, v: ImportView): boolean => (result !== null && result !== undefined) || v.size !== null;

/**
 * The person typed while a file may still be read: what they type is the field, so the read that is under way goes
 * stale (it would otherwise replace the text with the file's), and the view is the typed one.
 */
export function edited(guard: ReturnType<typeof latest>, v: ImportView, text: string): ImportView {
  guard.cancel();
  return typed(v, text);
}

/** A file was read into the field. `size` is the words for its size. */
export const fileRead = (size: string, text: string): ImportView => ({ text, note: null, size });

/** A file was not read: the field stays as it was (and so does the file it came from), with the reason in place of the note. */
export const fileRefused = (v: ImportView, error: string): ImportView => ({ ...v, note: { ok: false, text: error } });

/**
 * An import was applied: the field and the note start again (a second press would only be refused as a duplicate, and
 * the note would still say a file was read). A dialog closed without an import keeps both: Escape is not a Clear.
 */
export const applied = (): ImportView => FRESH;

/**
 * Tells the app layer the name of the file that was read. A name the bridge cannot carry (a lone surrogate, which JSON writes as an
 * escape that the app layer's parser refuses) must not stop the text and the note: it goes as a name that is nothing, which the app
 * layer keeps as empty and the page words. Nothing here cleans the name; the app layer does.
 */
export function sendSource(dispatch: (a: Action) => void, name: string): void {
  try {
    dispatch({ type: 'chooseVariablesImportSource', name });
  } catch {
    dispatch({ type: 'chooseVariablesImportSource', name: '' });
  }
}

/**
 * Whether the focus is on nothing that can take a key: nowhere, on the page body, or on a control that has become disabled
 * (the Import and Clear buttons are, once an import empties the field). In a modal dialog that leaves the keyboard with
 * nothing to act on, so the dialog moves the focus to a control that works.
 */
export const stranded = (active: { disabled?: boolean } | null | undefined, body: unknown): boolean => !active || active === body || active.disabled === true;

/**
 * The latest of a series of things that finish later, such as a file being read: `start` numbers one, `current` says whether
 * it is still the one that counts, and `cancel` makes every one that is under way stale (a dialog that closed, a Clear).
 */
export function latest(): { start: () => number; current: (token: number) => boolean; cancel: () => void } {
  let n = 0;
  return { start: () => ++n, current: (token) => token === n, cancel: () => void ++n };
}

/**
 * A file chosen for the field, read by `reader`: the view to take when it is read, or null when it went stale on the way
 * (the dialog closed, a Clear, another file chosen), so that a late result never fills a field the person has left.
 */
export async function fileInto<F extends { size: number }>(
  guard: ReturnType<typeof latest>,
  file: F,
  now: () => ImportView,
  reader: (f: F) => Promise<{ ok: true; text: string } | { ok: false; error: string }>,
  sizeText: (bytes: number) => string,
): Promise<ImportView | null> {
  const token = guard.start();
  const read = await reader(file);
  if (!guard.current(token)) return null;
  return read.ok ? fileRead(sizeText(file.size), read.text) : fileRefused(now(), read.error);
}
