// The Variables panel: what its input becomes. The rules (the list, the search, the checks of a value, an alias or a
// binding) are in the app layer; these functions only turn a field or a click into the action to send. Pure.
import type { Action, BindField, CollectionRow, ModeValue, VariableRow, VariablesPanel } from '@meldframe/protocol';
import { nameOf } from '@meldframe/web-document/names';

/** The text typed in a value field of one mode, as typed: the app layer sends nothing for empty text or a value the variable has already, and reads the rest by the type of the variable. */
export const valueAction = (row: VariableRow, mode: string, raw: string): Action => ({ type: 'typed', entry: { type: 'variableValue', variable: row.id, mode, text: raw } });

/** The button of a field: bind the variable to it, or let it go when it is bound already. */
export function bindAction(target: string, row: VariableRow, field: BindField, bound: boolean): Action {
  return { type: 'bindProperty', id: target, field, variable: bound ? null : row.id };
}

/** The button of a mode: show the collection in it. */
export function modeAction(c: CollectionRow, mode: string): Action {
  return { type: 'setMode', collection: c.id, mode };
}

/** A colour the swatch can draw: three, four, six or eight hex digits, the lengths the core reads (not five or seven). */
const HEX = /^#(?:[0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$/i;

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
export const modeName = (c: CollectionRow, id: string): string => {
  const found = c.modes.find((m) => m.id === id);
  return found ? nameOf(found.name) : id;
};

/** What opening the manage view sends first: it reads the whole list, so a search is cleared (the counts of a removal are of what is listed). */
export const beforeManage = (p: VariablesPanel): Action | null => (p.query.trim() === '' ? null : { type: 'setVariablesQuery', query: '' });
