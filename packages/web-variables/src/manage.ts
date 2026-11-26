// The Variables manage view: what its input becomes and what it says. The rules of a name are the core's (`valid_name`
// and `Limits`), checked here before anything is sent so the person is told in the field's own words; the app layer
// checks again and its refusals are worded in one table (web-document `notices.ts`). The view holds no business state:
// the collections, modes and variables are `AppState.variables`; this file only turns input into actions. Pure.
import type { Action, CollectionRow, Event, Mode, VarType, VariableEntity, VariableRow, VariablesPanel } from '@meldframe/protocol';
import { nameOf } from '@meldframe/web-document/names';
import { NAME_RULE_WORDS, noticeFor } from '@meldframe/web-document/notices';

/** The longest name of a variable or a collection, in characters (`max_name` of the core's limits). */
export const NAME_MAX = 256;
/** The longest name of a mode this view accepts: the core takes up to `NAME_MAX`, a mode is a short label. */
export const LABEL_MAX = 64;

/** The most collections in a file, modes in a collection and variables in a file (the core's `Limits`; a test reads them from its source). */
export const MAX_COLLECTIONS = 32;
export const MAX_MODES = 16;
export const MAX_VARIABLES = 5000;

/**
 * What a typed name may not hold: control, direction, zero-width and line-separator characters, the Hangul fillers, the Khmer
 * inherent vowels, the combining grapheme joiner, the blank braille cell and every space but the plain one. This checks what
 * the person types, before anything is sent; it is not a cleaner, and nothing in this view cleans a text it draws (the app
 * layer made every name of `state.variables` safe to show, rule U9). The name rules belong in the app layer (finding T5 of the
 * thin-UI audit); until they move, this check stays, and the test of the view packages lists it as the one exception.
 */
const HIDDEN = new RegExp('[\\p{Cc}\\p{Cf}\\p{Zl}\\p{Zp}\\u115F\\u1160\\u2800\\u3164\\uFFA0]|\\u034F|\\u17B4|\\u17B5|(?! )\\p{Zs}', 'u');

export type NameKind = 'collection' | 'mode' | 'variable';

export interface NameCheck {
  /** The name to send: the text without the spaces around it. */
  name: string;
  /** Why it cannot be sent; the core would refuse it, or it would be empty or unreadable. */
  error: string | null;
  /** A name the core takes but that the person probably did not mean. */
  warning: string | null;
}

/** The core rule of a variable or collection name: lowercase segments of letters, digits and single hyphens, joined by `/`. */
function isPath(name: string): boolean {
  const segment = (s: string) => s !== '' && !s.startsWith('-') && !s.endsWith('-') && !s.includes('--') && /^[a-z0-9-]+$/.test(s);
  return name.split('/').every(segment);
}

const KIND_WORDS: Record<NameKind, string> = { collection: 'collection', mode: 'mode', variable: 'variable' };

/**
 * Whether a typed name can be sent. A variable or collection is a lowercase ASCII slash path (the core refuses
 * anything else); a mode is free text of up to `LABEL_MAX` characters. None holds a control or direction character.
 * A variable that exists in the collection is an error (the core refuses it); a collection or a mode that exists is only
 * a warning, since the core accepts it (the ids differ). `existing` holds the names that are there now.
 */
export function checkName(kind: NameKind, raw: string, existing: readonly string[]): NameCheck {
  const name = raw.trim();
  const fail = (error: string): NameCheck => ({ name, error, warning: null });
  if (name === '') return fail(`Give the ${KIND_WORDS[kind]} a name.`);
  if (HIDDEN.test(name)) return fail('A name cannot hold control, direction or invisible characters.');
  const max = kind === 'mode' ? LABEL_MAX : NAME_MAX;
  if ([...name].length > max) return fail(`Use ${max} characters or fewer.`);
  if (kind !== 'mode' && !isPath(name)) return fail(NAME_RULE_WORDS);
  if (kind === 'variable') {
    return existing.includes(name) ? fail('A variable in this collection already has this name.') : { name, error: null, warning: null };
  }
  const same = existing.some((e) => e.trim().toLowerCase() === name.toLowerCase());
  const where = kind === 'mode' ? 'in this collection' : 'in this file';
  return { name, error: null, warning: same ? `A ${kind} with this name already exists ${where}. Both are kept, so tell them apart.` : null };
}

export const createCollectionAction = (name: string): Action => ({ type: 'createCollection', name: name.trim() });
export const addModeAction = (collection: string, name: string, copyFrom: string | null): Action =>
  copyFrom ? { type: 'addMode', collection, name: name.trim(), copyFrom } : { type: 'addMode', collection, name: name.trim() };
export const removeModeAction = (collection: string, mode: string): Action => ({ type: 'removeMode', collection, mode });
export const addVariableAction = (collection: string, name: string, kind: VarType): Action => ({ type: 'addVariable', collection, name: name.trim(), kind });
export const removeVariableAction = (id: string, detach: boolean): Action => ({ type: 'removeVariable', id, detach });
export const removeCollectionAction = (id: string, detach: boolean): Action => ({ type: 'removeCollection', id, detach });

export interface Summary {
  /** Variables of the collection (`CollectionRow.matched`: all of them while the search is empty). */
  variables: number;
  /** The uses of the variables that are listed: node fields and aliases. */
  uses: number;
  modes: number;
  /** Every variable of the collection is listed, so `uses` is the whole count; otherwise it is a lower bound. */
  complete: boolean;
}

/** What the header of a collection says, and what a removal of it would touch. The rows are the panel's (capped, and narrowed by the search). */
export function collectionSummary(c: CollectionRow, panel: VariablesPanel): Summary {
  return {
    variables: c.matched,
    uses: c.variables.reduce((n, v) => n + v.usedBy, 0),
    modes: c.modes.length,
    complete: panel.query.trim() === '' && c.variables.length === c.matched,
  };
}

/** What the person asked to remove, by id: the view shows the question for it and looks the thing up again when it answers. */
export type Ask = { kind: 'variable'; id: string } | { kind: 'collection'; id: string } | { kind: 'mode'; collection: string; id: string };

export type RemovalTarget = { kind: 'variable'; row: VariableRow } | { kind: 'collection'; col: CollectionRow; summary: Summary } | { kind: 'mode'; collection: string; mode: Mode };

export interface RemovalPlan {
  /** What to say; one question, and what each answer does. */
  text: string;
  /** The label of the button that does it: Detach when something is let go first, Remove when nothing is. */
  confirm: 'Remove' | 'Detach';
  action: Action;
  /** The status line once it was done. */
  done: string;
}

/** A number and its noun, singular for one: `1 use`, `2 uses`. */
export const count = (n: number, one: string, many: string): string => `${n} ${n === 1 ? one : many}`;
const quote = (name: string) => `“${nameOf(name)}”`;

/**
 * The confirmation of a removal. A variable nothing uses, an empty collection and a mode go with a plain removal (so
 * a use that appeared meanwhile is refused with E_VARIABLE_IN_USE, not detached unasked); anything that is used, or a
 * collection that holds variables, is removed with `detach: true`, one undo step (docs/tokens-and-variables.md 5.2).
 */
export function removalPlan(t: RemovalTarget): RemovalPlan {
  if (t.kind === 'mode') {
    const name = quote(t.mode.name);
    return { text: `Remove the mode ${name}? Its values go; the other modes keep theirs. Undo brings it back.`, confirm: 'Remove', action: removeModeAction(t.collection, t.mode.id), done: `Removed the mode ${name}.` };
  }
  if (t.kind === 'variable') {
    const name = quote(t.row.name);
    const n = t.row.usedBy;
    if (n === 0) return { text: `Remove ${name}? Nothing uses it. Undo brings it back.`, confirm: 'Remove', action: removeVariableAction(t.row.id, false), done: `Removed ${name}.` };
    return {
      text: `${name} has ${count(n, 'use', 'uses')} (node fields and aliases). Detach turns each use into a plain value (a node takes the value of the default mode), then removes the variable. One undo puts it all back.`,
      confirm: 'Detach',
      action: removeVariableAction(t.row.id, true),
      done: `Detached ${count(n, 'use', 'uses')} and removed ${name}.`,
    };
  }
  const name = quote(t.col.name);
  const { variables, uses, complete } = t.summary;
  if (variables === 0) return { text: `Remove the empty collection ${name}? Undo brings it back.`, confirm: 'Remove', action: removeCollectionAction(t.col.id, false), done: `Removed the collection ${name}.` };
  const used =
    uses === 0
      ? complete
        ? 'Nothing uses them.'
        : 'None of the variables listed here is used, but the list is cut, so others may be.'
      : `There ${uses === 1 && complete ? 'is' : 'are'} ${complete ? '' : 'at least '}${count(uses, 'use', 'uses')} in the file (node fields and aliases; aliases among these variables count too).`;
  return {
    text: `Remove the collection ${name} with ${count(variables, 'variable', 'variables')}? ${used} Detach turns every use into a plain value, then removes the variables and the collection. One undo puts it all back.`,
    confirm: 'Detach',
    action: removeCollectionAction(t.col.id, true),
    done: `Removed the collection ${name} with ${count(variables, 'variable', 'variables')}.`,
  };
}

export interface Reaction {
  /** The words of a refusal of this dispatch, or null. */
  error: string | null;
  /** The thing the dispatch made with an id of its own, to select and focus. */
  added: { entity: VariableEntity; id: string } | null;
  /** The words to announce for a change that was made (the `pending` text), or null for a refusal or for events that changed nothing. */
  status: string | null;
}

/**
 * The words of a limit or an id clash for what the view sent. The core uses E_TOO_LONG, E_TOO_MANY_ENTRIES and E_DUPLICATE_ID
 * for nodes and texts too, so the table of web-document words them without naming variables; here the view knows which
 * addition it sent and says which limit it met. Null for any other code, or when what was sent is not an addition.
 */
function sentWords(code: string, sent: Action | null): string | null {
  if (!sent || (sent.type !== 'createCollection' && sent.type !== 'addMode' && sent.type !== 'addVariable')) return null;
  if (code === 'E_TOO_LONG') return `That name is too long. Use ${NAME_MAX} characters or fewer.`;
  if (code === 'E_DUPLICATE_ID') return 'Another change took the same id at the same moment, so nothing was added. Try again.';
  if (code !== 'E_TOO_MANY_ENTRIES') return null;
  if (sent.type === 'createCollection') return `A file holds at most ${MAX_COLLECTIONS} collections.`;
  if (sent.type === 'addMode') return `A collection holds at most ${MAX_MODES} modes.`;
  return `A file holds at most ${MAX_VARIABLES.toLocaleString('en-US')} variables.`;
}

/** The words of the last refusal among `events`, for what the view sent (`sent`, null when it is not known). */
function refusalWords(events: readonly Event[], sent: Action | null): string | null {
  for (let i = events.length - 1; i >= 0; i--) {
    const e = events[i]!;
    const text = noticeFor(e);
    if (text === null) continue;
    return (e.type === 'refused' || e.type === 'opRejected' ? sentWords(e.code, sent) : null) ?? text;
  }
  return null;
}

/** What the view does with the events of a dispatch: say the refusal, find what was added, announce what was done. */
export function reactTo(events: readonly Event[], pending: string | null, sent: Action | null = null): Reaction {
  const error = refusalWords(events, sent);
  const made = [...events].reverse().find((e) => e.type === 'entityAdded');
  const changed = events.some((e) => e.type === 'documentChanged');
  return {
    error,
    added: made && made.type === 'entityAdded' ? { entity: made.entity, id: made.id } : null,
    status: !error && changed ? pending : null,
  };
}

/** The id of the collection that holds what was added, or null when it is not listed. */
export function whereAdded(panel: VariablesPanel, added: { entity: VariableEntity; id: string }): string | null {
  const found = panel.collections.find((c) => {
    if (added.entity === 'collection') return c.id === added.id;
    if (added.entity === 'mode') return c.modes.some((m) => m.id === added.id);
    return c.variables.some((v) => v.id === added.id);
  });
  return found?.id ?? null;
}

/** Why the counts of the view may be partial: a search narrows the panel's list, or the list holds at most so many variables. */
export function partial(panel: VariablesPanel): 'search' | 'cut' | null {
  if (panel.query.trim() !== '') return 'search';
  return panel.collections.some((c) => c.matched > c.variables.length) ? 'cut' : null;
}

/** What an `Ask` is about now, or null when it is not in the panel any more (someone removed it). */
export function targetOf(panel: VariablesPanel, ask: Ask): RemovalTarget | null {
  if (ask.kind === 'variable') {
    for (const c of panel.collections) {
      const row = c.variables.find((v) => v.id === ask.id);
      if (row) return { kind: 'variable', row };
    }
    return null;
  }
  const col = panel.collections.find((c) => c.id === (ask.kind === 'mode' ? ask.collection : ask.id));
  if (!col) return null;
  if (ask.kind === 'collection') return { kind: 'collection', col, summary: collectionSummary(col, panel) };
  const mode = col.modes.find((m) => m.id === ask.id);
  return mode ? { kind: 'mode', collection: col.id, mode } : null;
}
