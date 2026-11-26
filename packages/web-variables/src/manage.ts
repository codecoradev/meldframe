// The Variables manage view: what it says. The rules are the app layer's (audit finding T5): whether a name can be sent,
// the limits, what a removal detaches, and the rename flow are `AppState.manage` and `Action.manage`; this file holds the
// words of each code and number it is given, and what the view does with the events of a dispatch. Pure.
import type { Action, CollectionRow, Event, ManageLimits, NameDraft, NameError, NameWarning, Removal, VariableEntity, VariablesPanel } from '@meldframe/protocol';
import { nameOf } from '@meldframe/web-document/names';
import { NAME_RULE_WORDS, noticeFor } from '@meldframe/web-document/notices';

/**
 * The most collections in a file, modes in a collection and variables in a file, copied from the core's `Limits` for the
 * import view (`web-variables-io`), which has no state to read them from; a test reads them from the Rust source (finding T3).
 * The manage view reads them from `state.manage.limits`.
 */
export const MAX_COLLECTIONS = 32;
export const MAX_MODES = 16;
export const MAX_VARIABLES = 5000;

/** What a name is of: a collection, a mode or a variable. */
export type NameKind = VariableEntity;

/** The words of a name the app layer says cannot be sent. */
export function nameError(kind: NameKind, error: NameError, limits: ManageLimits): string {
  switch (error) {
    case 'empty':
      return `Give the ${kind} a name.`;
    case 'hidden':
      return 'A name cannot hold control, direction or invisible characters.';
    case 'tooLong':
      return `Use ${kind === 'mode' ? limits.modeNameMax : limits.nameMax} characters or fewer.`;
    case 'notPath':
      return NAME_RULE_WORDS;
    case 'duplicate':
      return 'A variable in this collection already has this name.';
  }
}

/** The words of a name the core takes but the person probably did not mean. */
export function nameWarning(kind: NameKind, warning: NameWarning): string {
  const where = kind === 'mode' ? 'in this collection' : 'in this file';
  return warning === 'same' ? `A ${kind} with this name already exists ${where}. Both are kept, so tell them apart.` : '';
}

/** Whether the person typed anything into a field: only then is a refusal of the name shown in it. */
export const typed = (text: string): boolean => text.trim() !== '';

export type NoteTone = 'error' | 'warning' | 'hint';

/** The line under a field: what it says and in which tone. */
export interface Note {
  tone: NoteTone;
  text: string;
}

/** The classes of each tone of a note (whole class names, so that Tailwind finds them). */
export const NOTE_CLASS: Record<NoteTone, string> = { error: 'text-danger-text', warning: 'text-text-strong', hint: 'text-text-muted' };

/** What the line under a form that adds something says about its text (`AppState.manage.drafts`); null for a mode with nothing to say. */
export function addNote(kind: NameKind, draft: NameDraft, limits: ManageLimits): Note | null {
  const check = draft.check;
  if (typed(draft.text) && check.error) return { tone: 'error', text: nameError(kind, check.error, limits) };
  if (check.warning) return { tone: 'warning', text: nameWarning(kind, check.warning) };
  return kind === 'mode' ? null : { tone: 'hint', text: NAME_RULE_WORDS };
}

export interface Summary {
  /** Variables of the collection (`CollectionRow.matched`: all of them while the search is empty). */
  variables: number;
  /** The uses of the variables that are listed: node fields and aliases. */
  uses: number;
  modes: number;
  /** Every variable of the collection is listed, so `uses` is the whole count; otherwise it is a lower bound. */
  complete: boolean;
}

/** What the header of a collection says. The rows are the panel's (capped, and narrowed by the search). */
export function collectionSummary(c: CollectionRow, panel: VariablesPanel): Summary {
  return {
    variables: c.matched,
    uses: c.variables.reduce((n, v) => n + v.usedBy, 0),
    modes: c.modes.length,
    complete: panel.query.trim() === '' && c.variables.length === c.matched,
  };
}

export interface RemovalWords {
  /** What to say; one question, and what each answer does. */
  text: string;
  /** The label of the button that does it: Detach when something is let go first, Remove when nothing is. */
  confirm: 'Remove' | 'Detach';
  /** The status line once it was done. */
  done: string;
}

/** A number and its noun, singular for one: `1 use`, `2 uses`. */
export const count = (n: number, one: string, many: string): string => `${n} ${n === 1 ? one : many}`;
const quote = (name: string) => `“${nameOf(name)}”`;

/** The words of the question before a removal, for what the app layer says it touches (`Removal`: the name, whether it detaches, the uses and the variables). */
export function removalWords(r: Removal): RemovalWords {
  const name = quote(r.name);
  if (r.target.kind === 'mode') return { text: `Remove the mode ${name}? Its values go; the other modes keep theirs. Undo brings it back.`, confirm: 'Remove', done: `Removed the mode ${name}.` };
  if (r.target.kind === 'variable') {
    if (!r.detach) return { text: `Remove ${name}? Nothing uses it. Undo brings it back.`, confirm: 'Remove', done: `Removed ${name}.` };
    return {
      text: `${name} has ${count(r.uses, 'use', 'uses')} (node fields and aliases). Detach turns each use into a plain value (a node takes the value of the default mode), then removes the variable. One undo puts it all back.`,
      confirm: 'Detach',
      done: `Detached ${count(r.uses, 'use', 'uses')} and removed ${name}.`,
    };
  }
  if (!r.detach) return { text: `Remove the empty collection ${name}? Undo brings it back.`, confirm: 'Remove', done: `Removed the collection ${name}.` };
  const used = r.uses === 0 ? 'Nothing uses them.' : `There ${r.uses === 1 ? 'is' : 'are'} ${count(r.uses, 'use', 'uses')} in the file (node fields and aliases; aliases among these variables count too).`;
  return {
    text: `Remove the collection ${name} with ${count(r.variables, 'variable', 'variables')}? ${used} Detach turns every use into a plain value, then removes the variables and the collection. One undo puts it all back.`,
    confirm: 'Detach',
    done: `Removed the collection ${name} with ${count(r.variables, 'variable', 'variables')}.`,
  };
}

/** The status line once a rename was done (`Event.renamed`); names drawn as everywhere else. */
export function renamedWords(e: { entity: VariableEntity; from: string; to: string }): string {
  const what = e.entity === 'variable' ? '' : `the ${e.entity} `;
  return `Renamed ${what}${quote(e.from)} to ${quote(e.to)}.`;
}

export interface Reaction {
  /** The words of a refusal of this dispatch, or null. */
  error: string | null;
  /** The thing the dispatch made with an id of its own, to focus. */
  added: { entity: VariableEntity; id: string } | null;
  /** A rename this dispatch made, to give the focus back to the button that asked for it. */
  renamed: { entity: VariableEntity; from: string; to: string } | null;
  /** The words to announce for a change that was made (the rename's, or the `pending` text), or null for a refusal or for events that changed nothing. */
  status: string | null;
}

/**
 * The words of a limit or an id clash for what the view sent. The core uses E_TOO_LONG, E_TOO_MANY_ENTRIES and E_DUPLICATE_ID
 * for nodes and texts too, so the table of web-document words them without naming variables; here the view knows which
 * addition it sent and says which limit it met. Null for any other code, or when what was sent is not an addition.
 */
function sentWords(code: string, sent: Action | null, limits: ManageLimits): string | null {
  if (!sent || (sent.type !== 'createCollection' && sent.type !== 'addMode' && sent.type !== 'addVariable')) return null;
  if (code === 'E_TOO_LONG') return `That name is too long. Use ${limits.nameMax} characters or fewer.`;
  if (code === 'E_DUPLICATE_ID') return 'Another change took the same id at the same moment, so nothing was added. Try again.';
  if (code !== 'E_TOO_MANY_ENTRIES') return null;
  if (sent.type === 'createCollection') return `A file holds at most ${limits.collections} collections.`;
  if (sent.type === 'addMode') return `A collection holds at most ${limits.modes} modes.`;
  return `A file holds at most ${limits.variables.toLocaleString('en-US')} variables.`;
}

/** The words of the last refusal among `events`, for what the view sent (`sent`, null when it is not known). */
function refusalWords(events: readonly Event[], sent: Action | null, limits: ManageLimits): string | null {
  for (let i = events.length - 1; i >= 0; i--) {
    const e = events[i]!;
    const text = noticeFor(e);
    if (text === null) continue;
    return (e.type === 'refused' || e.type === 'opRejected' ? sentWords(e.code, sent, limits) : null) ?? text;
  }
  return null;
}

/** What the view does with the events of a dispatch: say the refusal, find what was added, announce what was done. */
export function reactTo(events: readonly Event[], pending: string | null, limits: ManageLimits, sent: Action | null = null): Reaction {
  const error = refusalWords(events, sent, limits);
  const made = [...events].reverse().find((e) => e.type === 'entityAdded');
  const done = [...events].reverse().find((e) => e.type === 'renamed');
  const changed = events.some((e) => e.type === 'documentChanged');
  const renamed = done && done.type === 'renamed' ? { entity: done.entity, from: done.from, to: done.to } : null;
  return {
    error,
    added: made && made.type === 'entityAdded' ? { entity: made.entity, id: made.id } : null,
    renamed,
    status: error ? null : renamed ? renamedWords(renamed) : changed ? pending : null,
  };
}

/** Why the counts of the view may be partial: a search narrows the panel's list, or the list holds at most so many variables. */
export function partial(panel: VariablesPanel): 'search' | 'cut' | null {
  if (panel.query.trim() !== '') return 'search';
  return panel.collections.some((c) => c.matched > c.variables.length) ? 'cut' : null;
}
