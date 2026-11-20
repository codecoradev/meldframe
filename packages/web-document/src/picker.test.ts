import type { Picker, PickerRow, Properties } from '@meldframe/protocol';
import { describe, expect, it } from 'vitest';
import { closePickerAction, openPickerAction, pickAction, pickerQueryAction } from './picker';

const p = { id: 'n' } as Properties;
const picker: Picker = { id: 'n', field: 'fill', type: 'color', query: '', total: 1, rows: [] };
const row = (bound: boolean): PickerRow => ({ id: 'v', name: 'color/bg', collection: 'c', type: 'color', resolved: '#fff', bound });

describe('the bind picker', () => {
  it('opens on a field of the node, closes, and searches', () => {
    expect(openPickerAction(p, 'opacity')).toEqual({ type: 'openPicker', id: 'n', field: 'opacity' });
    expect(closePickerAction()).toEqual({ type: 'closePicker' });
    expect(pickerQueryAction('bg')).toEqual({ type: 'setPickerQuery', query: 'bg' });
  });
  it('binds a row and lets a bound row go', () => {
    expect(pickAction(picker, row(false))).toEqual({ type: 'bindProperty', id: 'n', field: 'fill', variable: 'v' });
    expect(pickAction(picker, row(true))).toEqual({ type: 'bindProperty', id: 'n', field: 'fill', variable: null });
  });
});
