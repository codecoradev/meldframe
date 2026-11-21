import { readFileSync } from 'node:fs';
import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type { CollectionRow, VariableRow, VariablesPanel } from '@meldframe/protocol';
import CollectionDetail from './CollectionDetail.svelte';
import RenameField from './RenameField.svelte';
import { collectionSummary, type Ask } from './manage';
import { RENAME_HINT, renameContext, renameHint } from './rename';

const row = (over: Partial<VariableRow> = {}): VariableRow => ({ id: 'v1', name: 'color/bg', type: 'color', values: [], usedBy: 2, binds: [], ...over });
const modes = [{ id: 'm1', name: 'light' }, { id: 'm2', name: 'dark' }];
const col = (over: Partial<CollectionRow> = {}): CollectionRow => ({ id: 'c1', name: 'theme', modes, active: 'm1', matched: 2, variables: [row(), row({ id: 'v2', name: 'space/4', type: 'number', usedBy: 0 })], ...over });
const panel = (c: CollectionRow = col()): VariablesPanel => ({ query: '', total: 2, matched: 2, collections: [c] });
const noop = () => {};

/** The detail pane with the field of `renaming` open, or none. */
const detail = (c: CollectionRow, renaming: Ask | null) => {
  const p = panel(c);
  const props = { col: c, summary: collectionSummary(c, p), onAsk: noop, onAddMode: noop, onAddVariable: noop, modeKey: 0, variableKey: 0, renaming, ctx: renaming ? renameContext(p, renaming) : null, onRename: noop, onRenameSubmit: noop, onRenameCancel: noop };
  return render(CollectionDetail as never, { props: props as never }).body;
};
const field = (kind: 'variable' | 'collection' | 'mode', current: string, others: string[] = [], subject = current, first = false) =>
  render(RenameField as never, { props: { kind, subject, first, ctx: { current, others }, onSubmit: noop, onCancel: noop, testId: 'rename-field' } as never }).body;

describe('the Rename buttons', () => {
  const out = detail(col(), null);
  it('are labelled by their visible word first and name their target, for the collection, each mode and each row', () => {
    expect(out).toContain('aria-label="Rename collection theme"');
    expect(out).toContain('aria-label="Rename mode light"');
    expect(out).toContain('aria-label="Rename mode dark"');
    expect(out).toContain('aria-label="Rename color/bg"');
    expect(out).toContain('aria-label="Rename space/4"');
    expect(out.match(/>Rename</g)).toHaveLength(5);
  });
  it('are there for the default mode too (it keeps its place; only the name changes)', () => {
    expect(out).toContain('aria-label="Rename mode light"');
  });
  it('carry a key for the focus to come back to', () => {
    for (const key of ['collection:c1', 'mode:m1', 'mode:m2', 'variable:v1', 'variable:v2']) expect(out).toContain(`data-rename="${key}"`);
  });
  it('are drawn from the display name: markup and direction characters in a name do not reach the label', () => {
    const o = detail(col({ name: 'br‮and', variables: [row({ name: '<img src=x onerror=alert(1)>' })], matched: 1 }), null);
    expect(o).toContain('aria-label="Rename collection brand"');
    expect(o).not.toContain('<img src=x');
    expect(o).not.toMatch(/[‮]/);
  });
});

describe('the field of a rename', () => {
  it('is prefilled with the current name, labelled with its target, with Save and Cancel named for it', () => {
    const out = field('variable', 'color/bg');
    expect(out).toMatch(/<input[^>]*value="color\/bg"/);
    expect(out).toContain('aria-label="New name of color/bg"');
    expect(out).toContain('aria-label="Save the new name of color/bg"');
    expect(out).toContain('aria-label="Cancel renaming color/bg"');
    expect(out).toMatch(/>Save</);
    expect(out).toMatch(/>Cancel</);
  });
  it('says the rule of a variable or collection name and, once, what a rename does to the export', () => {
    const out = field('variable', 'color/bg');
    expect(out).toContain('lowercase letters, digits and single hyphens');
    expect(out).toContain(RENAME_HINT);
    expect(out.split(RENAME_HINT)).toHaveLength(2);
    expect(field('collection', 'theme')).toContain(RENAME_HINT);
    expect(field('mode', 'dark')).not.toContain('lowercase letters');
  });
  it('says for a mode what the export does with that mode: the first one is not named in it, a later one is', () => {
    const later = field('mode', 'dark');
    expect(later).toContain(renameHint('mode', false));
    expect(later).not.toContain(RENAME_HINT);
    const first = field('mode', 'light', [], 'light', true);
    expect(first).toContain(renameHint('mode', true));
    expect(first).toMatch(/not part of the exported names/);
    expect(first).not.toContain(renameHint('mode', false));
  });
  it('keeps a place for the words about a name that changed meanwhile, as an empty status region', () => {
    expect(field('variable', 'color/bg')).toMatch(/<p[^>]*role="status"[^>]*data-testid="rename-field-moved"[^>]*><\/p>|<p[^>]*data-testid="rename-field-moved"[^>]*role="status"[^>]*><\/p>/);
  });
  it('shows the raw name in the field as text: markup is escaped, and the neutralised label is used for the names around it', () => {
    const out = field('mode', '<b>da‮rk</b>', [], 'mode dark');
    expect(out).toContain('value="&lt;b>da');
    expect(out).not.toContain('<b>da');
    expect(out).toContain('aria-label="New name of mode dark"');
  });
});

describe('the detail pane while a field is open', () => {
  it('shows the field in place of the collection title, and no Rename button for it', () => {
    const out = detail(col(), { kind: 'collection', id: 'c1' });
    expect(out).toContain('aria-label="New name of collection theme"');
    expect(out).not.toContain('aria-label="Rename collection theme"');
    expect(out).toContain('aria-label="Rename mode light"');
    expect(out).toContain('aria-label="Rename color/bg"');
  });
  it('tells the detail pane which mode is the first, so its field says what is true of it', () => {
    const first = detail(col(), { kind: 'mode', collection: 'c1', id: 'm1' });
    expect(first).toContain(renameHint('mode', true));
    const later = detail(col(), { kind: 'mode', collection: 'c1', id: 'm2' });
    expect(later).toContain(renameHint('mode', false));
  });
  it('shows the field in place of the chip text of a mode', () => {
    const out = detail(col(), { kind: 'mode', collection: 'c1', id: 'm2' });
    expect(out).toContain('aria-label="New name of mode dark"');
    expect(out).toMatch(/<input[^>]*value="dark"/);
    expect(out).not.toContain('aria-label="Rename mode dark"');
    expect(out).toContain('aria-label="Rename mode light"');
  });
  it('shows the field in the name cell of the row, and keeps the row header and the other buttons', () => {
    const out = detail(col(), { kind: 'variable', id: 'v1' });
    expect(out).toMatch(/<input[^>]*value="color\/bg"/);
    expect(out).toContain('aria-label="New name of color/bg"');
    expect(out).not.toContain('aria-label="Rename color/bg"');
    expect(out).toContain('aria-label="Remove color/bg"');
    expect(out).toContain('aria-label="Rename space/4"');
    expect(out).toContain('scope="row"');
  });
  it('opens only one field: a field for a thing that is not in this collection draws none', () => {
    const out = detail(col(), { kind: 'variable', id: 'elsewhere' });
    expect(out).not.toContain('aria-label="New name of');
  });
});

describe('the manage view with a rename', () => {
  it('never uses markup from a name in the new files either', () => {
    const read = (f: string) => readFileSync(new URL(`./${f}`, import.meta.url), 'utf8');
    for (const f of ['RenameField.svelte', 'rename.ts']) expect(read(f), f).not.toContain('{@html');
  });
});
