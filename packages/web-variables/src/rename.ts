// Renaming in the Variables manage view: what a rename is checked against, the one action it sends and what it says.
// The rules of a name are those of an addition (`checkName`); only the name the thing has now is left out of the names
// it is compared with, and the name it has now is not a change. Ids stay, so bindings and aliases keep pointing at it.
// The view holds no business state: the target is an id and `renameContext` reads it again from the panel. Pure.
import type { Action, VariablesPanel } from '@meldframe/protocol';
import { noticeFor } from '@meldframe/web-document/notices';
import { checkName, nameOf, type Ask, type NameCheck, type NameKind } from './manage';

/**
 * What a rename of a collection or a variable does to the export, said once in the field (docs/tokens-and-variables.md 9.6: export names change, the next
 * export reflects the new name, a breaking change for code that uses the CSS variable; ADR-0019: a rename of a collection, a
 * variable or a mode changes the names in the next export; ADR-0017: the ids stay).
 */
export const RENAME_HINT = 'The next export uses the new name. Code that uses the old name has to change. Bindings and aliases keep working.';
const ALIASES = 'Bindings and aliases keep working.';

/**
 * The words of the field about the export, for what is true of the kind. A variable is `--<collection>-<name>` and a
 * collection is in that name and in the attribute of its modes; a mode is exported only from the second one on, as the
 * value of `data-mode-<collection>` (the first is the `:root` rule: crates/features/export/export/src/vars.rs, ADR-0019).
 */
export function renameHint(kind: NameKind, first: boolean): string {
  if (kind !== 'mode') return RENAME_HINT;
  return first ? `The first mode is the default one; its name is not part of the exported names. ${ALIASES}` : `The next export uses the new name for this mode. ${ALIASES}`;
}

/** One undo step each: the app layer checks the list and applies it as one group. */
const edit = (command: Extract<Action, { type: 'editVariables' }>['commands'][number]): Action => ({ type: 'editVariables', commands: [command] });

export const renameCollectionAction = (id: string, name: string): Action => edit({ type: 'renameCollection', id, name: name.trim() });
export const renameModeAction = (collection: string, mode: string, name: string): Action => edit({ type: 'renameMode', collection, mode, name: name.trim() });
export const renameVariableAction = (id: string, name: string): Action => edit({ type: 'renameVariable', id, name: name.trim() });

/** The action that renames what `ask` names, by its kind. */
export function renameAction(ask: Ask, name: string): Action {
  if (ask.kind === 'collection') return renameCollectionAction(ask.id, name);
  if (ask.kind === 'mode') return renameModeAction(ask.collection, ask.id, name);
  return renameVariableAction(ask.id, name);
}

export interface RenameContext {
  /** The name it has now, as stored. */
  current: string;
  /** The names of the others of its kind: the variables of its collection, the collections of the file, the modes of its collection. */
  others: string[];
}

/** What the target of a rename is now, or null when another actor removed it (a mode goes with its collection). */
export function renameContext(panel: VariablesPanel, ask: Ask): RenameContext | null {
  if (ask.kind === 'variable') {
    for (const c of panel.collections) {
      const v = c.variables.find((x) => x.id === ask.id);
      if (v) return { current: v.name, others: c.variables.filter((x) => x.id !== ask.id).map((x) => x.name) };
    }
    return null;
  }
  if (ask.kind === 'collection') {
    const c = panel.collections.find((x) => x.id === ask.id);
    return c ? { current: c.name, others: panel.collections.filter((x) => x.id !== ask.id).map((x) => x.name) } : null;
  }
  const col = panel.collections.find((x) => x.id === ask.collection);
  const m = col?.modes.find((x) => x.id === ask.id);
  return col && m ? { current: m.name, others: col.modes.filter((x) => x.id !== ask.id).map((x) => x.name) } : null;
}

export interface RenameCheck extends NameCheck {
  /** The name is the one it has now: closing the field is the whole answer and nothing is sent (the core would accept it and record a useless undo step). */
  unchanged: boolean;
}

/**
 * Whether a typed name can be sent for a rename: the rules of an addition, without the own name among the names taken.
 * The text the field opened with (`opened`, the current name when none is given) is no change either: if another actor
 * renamed the thing meanwhile and the person typed nothing, sending it would undo that rename.
 */
export function checkRename(kind: NameKind, raw: string, ctx: RenameContext, opened: string = ctx.current): RenameCheck {
  const name = raw.trim();
  if (raw === ctx.current || name === ctx.current || raw === opened || name === opened) return { name, error: null, warning: null, unchanged: true };
  return { ...checkName(kind, raw, ctx.others), unchanged: false };
}

const quote = (name: string) => `“${nameOf(name)}”`;

/** The status line once a rename was done; names drawn as everywhere else. */
export function renamedWords(kind: NameKind, from: string, to: string): string {
  const what = kind === 'variable' ? '' : `the ${kind} `;
  return `Renamed ${what}${quote(from)} to ${quote(to)}.`;
}

/** What the field says when the name changed under it (another actor's rename), or null while it is the one the field opened with. */
export function movedWords(opened: string, ctx: RenameContext): string | null {
  return ctx.current === opened ? null : `This name changed to ${quote(ctx.current)} while you were editing.`;
}

/** The field of a rename and the rename that was sent from it; every way the field closes clears both, so an old marker cannot close a later field. */
export interface RenameFlow {
  field: Ask | null;
  sent: Ask | null;
}
export const NO_RENAME: RenameFlow = { field: null, sent: null };
export const openRename = (a: Ask): RenameFlow => ({ field: a, sent: null });
export const closeRename = (): RenameFlow => NO_RENAME;
/** The field's text was sent: its answer is awaited. Nothing is sent without a field. */
export const sendRename = (f: RenameFlow): RenameFlow => (f.field ? { field: f.field, sent: f.field } : f);

const same = (a: Ask, b: Ask): boolean => a.kind === b.kind && a.id === b.id && (a.kind !== 'mode' || (b.kind === 'mode' && a.collection === b.collection));

/** What an answer of the app does to the field: it closes only for the rename it sent, and only when that was made; the button then takes the focus. */
export function answerRename(f: RenameFlow, r: { error: string | null; status: string | null }): { flow: RenameFlow; refocus: Ask | null } {
  if (!f.sent || !f.field || !same(f.sent, f.field)) return { flow: f, refocus: null };
  if (r.error) return { flow: { field: f.field, sent: null }, refocus: null };
  if (!r.status) return { flow: f, refocus: null };
  return { flow: NO_RENAME, refocus: f.field };
}

/** The words for a target that was removed while its field was open: the ones of the table of refusals. */
export function goneWords(panel: VariablesPanel, ask: Ask): string {
  const code = ask.kind === 'variable' ? 'E_VARIABLE_NOT_FOUND' : ask.kind === 'collection' || !panel.collections.some((c) => c.id === ask.collection) ? 'E_COLLECTION_NOT_FOUND' : 'E_MODE_NOT_FOUND';
  return noticeFor({ type: 'refused', code }) ?? 'That no longer exists.';
}
