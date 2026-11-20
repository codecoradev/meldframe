import { describe, expect, it } from 'vitest';
import type { CollectionRow, VariableRow, VariablesPanel } from '@meldframe/protocol';
import { bindAction, display, listedCount, shownValue, modeAction, swatch, truncated, valueAction } from './variables';

const row = (over: Partial<VariableRow> = {}): VariableRow => ({
  id: 'bg',
  name: 'color/bg',
  type: 'color',
  values: [
    { mode: 'light', text: '#ffffff', resolved: '#ffffff' },
    { mode: 'dark', text: 'color/ink', alias: 'ink', resolved: '#000000' },
  ],
  usedBy: 1,
  binds: [{ field: 'fill', bound: false }],
  ...over,
});

describe('valueAction', () => {
  it('sends the typed text as a literal and leaves the checking to the app layer', () => {
    expect(valueAction(row(), 'light', '  #eeeeee ')).toEqual({
      type: 'setVariableValue',
      variable: 'bg',
      mode: 'light',
      value: { type: 'literal', text: '#eeeeee' },
    });
  });
  it('makes no action for the text the value already shows, or for no text', () => {
    expect(valueAction(row(), 'light', '#ffffff')).toBeNull();
    expect(valueAction(row(), 'light', '   ')).toBeNull();
    expect(valueAction(row(), 'nope', '#fff')).toBeNull();
  });
  it('replaces an alias with a literal, also when the text is the name it shows', () => {
    expect(valueAction(row(), 'dark', '#111111')).not.toBeNull();
    expect(valueAction(row(), 'dark', 'color/ink')).not.toBeNull();
  });
});

describe('bindAction', () => {
  it('binds an unbound field and unbinds a bound one', () => {
    expect(bindAction('n1', row(), 'fill', false)).toEqual({ type: 'bindProperty', id: 'n1', field: 'fill', variable: 'bg' });
    expect(bindAction('n1', row(), 'fill', true)).toEqual({ type: 'bindProperty', id: 'n1', field: 'fill', variable: null });
  });
});

describe('modeAction', () => {
  const c = { id: 't', name: 'appearance', modes: [{ id: 'light', name: 'light' }, { id: 'dark', name: 'dark' }], active: 'light', matched: 0, variables: [] } as CollectionRow;
  it('picks a mode of the collection', () => {
    expect(modeAction(c, 'dark')).toEqual({ type: 'setMode', collection: 't', mode: 'dark' });
  });
});

describe('swatch', () => {
  it('gives a colour only for a colour row with a hex value', () => {
    expect(swatch(row(), 'dark')).toBe('#000000');
    expect(swatch(row({ type: 'number' }), 'dark')).toBeNull();
    expect(swatch(row({ values: [{ mode: 'light', text: 'x', resolved: 'url(x)' }] }), 'light')).toBeNull();
    expect(swatch(row({ values: [{ mode: 'light', text: 'x', resolved: '' }] }), 'light')).toBeNull();
  });
});

describe('the list', () => {
  const panel = (matched: number, n: number): VariablesPanel => ({
    query: '',
    total: matched,
    matched,
    collections: [{ id: 't', name: 't', modes: [], active: '', matched: n, variables: Array.from({ length: n }, () => row()) }],
  });
  it('counts what is listed and says when the search matched more', () => {
    expect(listedCount(panel(5, 5))).toBe(5);
    expect(truncated(panel(5, 5))).toBe(false);
    expect(truncated(panel(250, 200))).toBe(true);
  });
});

describe('the value a row shows', () => {
  const c = { id: 'c', name: 'appearance', modes: [{ id: 'light', name: 'light' }, { id: 'dark', name: 'dark' }], active: 'dark', matched: 1, variables: [row()] } as CollectionRow;
  it('is the one of the mode on view, an alias as the name it points at', () => {
    const v = shownValue(c, row())!;
    expect(v.mode).toBe('dark');
    expect(display(v)).toBe('→ color/ink');
    expect(display(row().values[0]!)).toBe('#ffffff');
  });
  it('is nothing for a mode the row has no value in', () => {
    expect(shownValue({ ...c, active: 'sepia' }, row())).toBeUndefined();
  });
});
