// The Variables panel: what its input becomes. The rules (the list, the search, the checks of a value, an alias or a
// binding) are in the app layer; these functions only turn a field or a click into the action to send. Pure.
import type { Action, BindField, CollectionRow, ModeValue, VariableRow, VariablesPanel } from '@meldframe/protocol';

/** The text typed in a value field of one mode. Nothing to send for empty text or the text the value already shows. */
export function valueAction(row: VariableRow, mode: string, raw: string): Action | null {
  const current = row.values.find((v) => v.mode === mode);
  const text = raw.trim();
  if (!current || text === '') return null;
  // An alias shows the name of its target: typing that name is still a literal and replaces the alias.
  if (!current.alias && text === current.text) return null;
  return { type: 'setVariableValue', variable: row.id, mode, value: { type: 'literal', text } };
}

/** The button of a field: bind the variable to it, or let it go when it is bound already. */
export function bindAction(target: string, row: VariableRow, field: BindField, bound: boolean): Action {
  return { type: 'bindProperty', id: target, field, variable: bound ? null : row.id };
}

/** The button of a mode: show the collection in it. */
export function modeAction(c: CollectionRow, mode: string): Action {
  return { type: 'setMode', collection: c.id, mode };
}

const HEX = /^#[0-9a-f]{3,8}$/i;

/** The colour of the swatch of a value (what it comes to in its mode), or null for a number or a value that does not resolve. */
export function swatch(row: VariableRow, mode: string): string | null {
  if (row.type !== 'color') return null;
  const resolved = row.values.find((v) => v.mode === mode)?.resolved ?? '';
  return HEX.test(resolved) ? resolved : null;
}

/** How many variables the panel lists. */
export const listedCount = (p: VariablesPanel): number => p.collections.reduce((n, c) => n + c.variables.length, 0);

/** The search matched more variables than the panel lists. */
export const truncated = (p: VariablesPanel): boolean => p.matched > listedCount(p);

/** The value of a row in the mode the collection shows: the one the row's field edits. */
export const shownValue = (c: CollectionRow, row: VariableRow): ModeValue | undefined => row.values.find((v) => v.mode === c.active);

/** A value as its field shows it: an alias as the name of its target. */
export const display = (val: ModeValue): string => (val.alias ? `→ ${val.text}` : val.text);

/** The words of the field a bind button stands for (the words of a type, `TYPE_LABEL`, are in web-document). */
export const FIELD_LABEL: Record<BindField, string> = { fill: 'Fill', radius: 'Radius', opacity: 'Opacity' };

/** The name of a mode for a label (the id, for a mode the collection does not have). */
export const modeName = (c: CollectionRow, id: string): string => c.modes.find((m) => m.id === id)?.name ?? id;
