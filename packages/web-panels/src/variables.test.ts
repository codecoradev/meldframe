import { describe, expect, it } from 'vitest';
import type { CollectionRow, VariableRow, VariablesPanel } from '@meldframe/protocol';
import { beforeManage, bindAction, display, modeName, listedCount, shownValue, modeAction, swatch, truncated, valueAction } from './variables';

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
  // Empty text, a text the value already has and the check by the type of the variable are the app layer's (property_text.rs).
  it('sends the typed text as it is typed, for the app layer to read by the type of the variable', () => {
    expect(valueAction(row(), 'light', '  #eeeeee ')).toEqual({ type: 'typed', entry: { type: 'variableValue', variable: 'bg', mode: 'light', text: '  #eeeeee ' } });
    expect(valueAction(row(), 'dark', '   ')).toMatchObject({ type: 'typed', entry: { type: 'variableValue', mode: 'dark', text: '   ' } });
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
  it('draws the hex lengths the core reads (3, 4, 6, 8) and no other', () => {
    const of = (resolved: string) => swatch(row({ values: [{ mode: 'light', text: 'x', resolved }] }), 'light');
    for (const hex of ['#abc', '#abcd', '#abcdef', '#abcdef12']) expect(of(hex)).toBe(hex);
    for (const hex of ['#ab', '#abcde', '#abcdef1', '#abcdef123', '#ggg']) expect(of(hex)).toBeNull();
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

describe('modeName', () => {
  const c = { id: 'c', name: 'appearance', modes: [{ id: 'm1', name: 'Light' }, { id: 'm2', name: 'Dark' }], active: 'm1', matched: 0, variables: [] } as CollectionRow;
  it('is the name of a mode, not its id, and the id for a mode that is not in the collection', () => {
    expect(modeName(c, 'm2')).toBe('Dark');
    expect(modeName(c, 'gone')).toBe('gone');
  });
});

describe('beforeManage', () => {
  const panel = (query: string) => ({ query, total: 0, matched: 0, collections: [] }) as VariablesPanel;
  it('clears a search before the manage view opens, and sends nothing when there is none', () => {
    expect(beforeManage(panel('bg'))).toEqual({ type: 'setVariablesQuery', query: '' });
    expect(beforeManage(panel('  '))).toBeNull();
    expect(beforeManage(panel(''))).toBeNull();
  });
});
