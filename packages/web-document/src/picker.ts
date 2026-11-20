// The bind picker of a field: what opening it, searching it and picking a row send. The rules (which variables fit a
// field, the search, what a bind does to the picker) are the app layer's. Pure.
import type { Action, BindField, Picker, PickerRow, Properties } from '@meldframe/protocol';

export const openPickerAction = (p: Properties, field: BindField): Action => ({ type: 'openPicker', id: p.id, field });
export const closePickerAction = (): Action => ({ type: 'closePicker' });
export const pickerQueryAction = (query: string): Action => ({ type: 'setPickerQuery', query });
/** A row of the picker binds the field to its variable; a row that is bound already lets it go. */
export const pickAction = (picker: Picker, row: PickerRow): Action => ({ type: 'bindProperty', id: picker.id, field: picker.field, variable: row.bound ? null : row.id });
