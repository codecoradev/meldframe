// Renaming in the Variables manage view: what the field says. The rules (what a rename is checked against, which text is no
// change, the one action a save sends, what happens when the name changes under the field or its target goes) are the app
// layer's: `AppState.manage.rename` and `Action.manage`. Pure.
import type { ManageLimits, ManageTarget, RenameField } from '@meldframe/protocol';
import { nameOf } from '@meldframe/web-document/names';
import { NAME_RULE_WORDS } from '@meldframe/web-document/notices';
import { type NameKind, type Note, nameError, nameWarning } from './manage';

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

const quote = (name: string) => `“${nameOf(name)}”`;

/** What the field says when the name changed under it (another actor's rename), or null while it is the one the field opened with. */
export function movedWords(opened: string, current: string): string | null {
  return current === opened ? null : `This name changed to ${quote(current)} while you were editing.`;
}

/** What the line under the field of a rename says: a refusal of the text (not for text that is no change), a warning, or what the rename does to the export. */
export function renameNote(kind: NameKind, f: RenameField, limits: ManageLimits): Note {
  const check = f.check;
  if (!f.unchanged && check.error) return { tone: 'error', text: nameError(kind, check.error, limits) };
  if (check.warning) return { tone: 'warning', text: nameWarning(kind, check.warning) };
  return { tone: 'hint', text: kind === 'mode' ? renameHint(kind, f.first) : `${NAME_RULE_WORDS} ${renameHint(kind, f.first)}` };
}

/** Whether the field of a rename is open for this thing of the collection `collection` (a mode is told apart by its collection). */
export function isEditing(field: RenameField | null, collection: string, kind: ManageTarget['kind'], id: string): boolean {
  const t = field?.target;
  return !!t && t.kind === kind && t.id === id && (t.kind !== 'mode' || t.collection === collection);
}
