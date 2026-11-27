// The React detail pane, body and dialog of the manage view, checked as manage-view.test.ts and rename-view.test.ts check the Svelte
// ones: the markup of a server render, in Node, and compared with the Svelte copy's render token by token (`tokens`). What a server
// render cannot show (the dialog's portal, the focus, Escape, the keys, where the focus goes after a dispatch) is the job of
// packages/web-variables/scripts/manage-e2e.mjs with MELDFRAME_UI=react, and the real pixels that of packages/web-panels/scripts/pixels-e2e.mjs.
import type { CollectionRow, ManageState, ManageTarget, Removal, RenameField as RenameState, VariableRow, VariablesPanel } from '@meldframe/protocol';
import { readFileSync } from 'node:fs';
import { renderToStaticMarkup } from 'react-dom/server';
import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import CollectionDetailSvelte from '../CollectionDetail.svelte';
import ManageBodySvelte from '../ManageBody.svelte';
import { collectionSummary } from '../manage';
import { CollectionDetail } from './CollectionDetail';
import { ManageBody } from './ManageBody';
import { ManageDialog } from './ManageDialog';
import { tokens } from '@meldframe/web-chrome/react/markup.testutil';

const noop = () => {};
const row = (over: Partial<VariableRow> = {}): VariableRow => ({ id: 'v1', name: 'color/bg', type: 'color', values: [], usedBy: 2, binds: [], ...over });
const modes = [{ id: 'm1', name: 'light' }, { id: 'm2', name: 'dark' }];
const col = (over: Partial<CollectionRow> = {}): CollectionRow => ({ id: 'c1', name: 'theme', modes, active: 'm1', matched: 2, variables: [row(), row({ id: 'v2', name: 'space/4', type: 'number', usedBy: 0 })], ...over });
const brand = col({ id: 'c2', name: 'brand', modes: [{ id: 'm9', name: 'default' }], matched: 0, variables: [] });
const panel = (collections: CollectionRow[] = [col(), brand], over: Partial<VariablesPanel> = {}): VariablesPanel => ({ query: '', total: 2, matched: 2, collections, ...over });
const empty = { text: '', check: { name: '', error: 'empty' as const } };
const limits = { nameMax: 256, modeNameMax: 64, collections: 32, modes: 16, variables: 5000 };
const manageState = (over: Partial<ManageState> = {}): ManageState => ({ open: false, shown: 'c1', limits, drafts: { collection: empty, mode: empty, variable: empty }, ...over });
const renaming = (target: ManageTarget, current: string, over: Partial<RenameState> = {}): RenameState => ({ target, opened: current, current, text: current, check: { name: current }, unchanged: true, first: false, ...over });

const body = (p: VariablesPanel, m: ManageState = manageState()) => renderToStaticMarkup(<ManageBody panel={p} manage={m} events={[]} dispatch={noop} />);
const bodyS = (p: VariablesPanel, m: ManageState = manageState()) => render(ManageBodySvelte as never, { props: { panel: p, manage: m, events: [], dispatch: noop } as never }).body;
const detailProps = (c: CollectionRow, rename: RenameState | null, drafts = manageState().drafts) => ({
  col: c, summary: collectionSummary(c, panel([c])), onAsk: noop, onAddMode: noop, onAddVariable: noop, rename, drafts, limits, onRename: noop, onRenameInput: noop, onRenameSubmit: noop, onRenameCancel: noop, onDraft: noop,
});
const detail = (c: CollectionRow, rename: RenameState | null = null) => renderToStaticMarkup(<CollectionDetail {...detailProps(c, rename)} />);
const detailS = (c: CollectionRow, rename: RenameState | null = null) => render(CollectionDetailSvelte as never, { props: detailProps(c, rename) as never }).body;

describe('the React manage view', () => {
  it('lists the collections with their variables and modes, and opens on the shown one', () => {
    const out = body(panel());
    expect(out.match(/data-testid="manage-collection"/g)).toHaveLength(2);
    expect(out.match(/data-testid="manage-var"/g)).toHaveLength(2);
    expect(out.match(/data-testid="manage-mode"/g)).toHaveLength(2);
    expect(out).toMatch(/aria-current="true"[^>]*data-id="c1"|data-id="c1"[^>]*aria-current="true"/);
  });
  it('shows the type and the uses of each variable and the uses of the collection', () => {
    const out = body(panel());
    expect(out).toContain('>Color<');
    expect(out).toContain('>Number<');
    expect(out).toContain('2 variables · 2 uses · 2 modes');
    expect(out).toContain('scope="col"');
    expect(out).toContain('scope="row"');
  });
  it('marks the first mode as the default one and offers no removal for it, and a rename for every mode', () => {
    const out = body(panel());
    expect(out).toContain('>default<');
    expect(out).toContain('aria-label="Remove mode dark"');
    expect(out).not.toContain('aria-label="Remove mode light"');
    expect(out).toContain('aria-label="Rename mode light"');
  });
  it('has a labelled field for each of the three things that can be added, with Add off while it is empty', () => {
    const out = body(panel());
    for (const label of ['New collection name', 'New mode name', 'New variable name']) expect(out).toContain(`aria-label="${label}"`);
    expect(out.match(/<button[^>]*data-testid="manage-new-[a-z]+-add"[^>]*>/g)!.filter((b) => /\sdisabled(=|\s|>)/.test(b))).toHaveLength(3);
  });
  it('asks the type of a new variable and where a new mode takes its values from', () => {
    const out = body(panel());
    expect(out.match(/role="radio"/g)).toHaveLength(2);
    expect(out).toContain('aria-label="Type of the new variable"');
    expect(out).toContain('aria-label="Copy the values of the new mode from"');
    expect(out).toMatch(/<option[^>]*value="m1"[^>]*>light/);
  });
  it('draws names as text: markup in a name is escaped (the app layer made them safe to show, and nothing here cleans them)', () => {
    const out = body(panel([col({ name: 'brand', variables: [row({ name: '<img src=x onerror=alert(1)>' })], matched: 1, modes: [{ id: 'm1', name: 'light' }, { id: 'm2', name: '<b>dark</b>' }] })]));
    expect(out).not.toContain('<img src=x');
    expect(out).toContain('&lt;img src=x');
    expect(out).not.toContain('<b>dark');
  });
  it('says when the list is cut or narrowed by a search, so the counts are known to be partial, with Clear search for the search', () => {
    expect(body(panel())).not.toContain('data-testid="manage-partial"');
    expect(body(panel([col({ matched: 300 })]))).toMatch(/data-testid="manage-partial"[^>]*><span>This file has more variables/);
    expect(body(panel([col()], { query: 'bg' }))).toMatch(/data-testid="manage-clear-search"[^>]*>Clear search</);
  });
  it('says what to do in a file with no collection, and has no detail pane then', () => {
    const out = body(panel([]), manageState({ shown: undefined }));
    expect(out).toMatch(/no collections yet/i);
    expect(out).not.toContain('data-testid="manage-detail-body"');
  });
  it('keeps both live regions on the page, empty until something is said', () => {
    const out = body(panel());
    expect(out).toMatch(/role="alert"[^>]*data-testid="manage-error"[^>]*><\/div>/);
    expect(out).toMatch(/role="status"[^>]*data-testid="manage-status"[^>]*><\/div>/);
  });
  it('shows the collection the app layer says is shown, the question it holds, and the text of a form with its check', () => {
    expect(body(panel(), manageState({ shown: 'c2' }))).toMatch(/data-testid="manage-detail-body"[^>]*data-id="c2"|data-id="c2"[^>]*data-testid="manage-detail-body"/);
    expect(body(panel())).not.toContain('data-testid="remove-confirm"');
    const removal: Removal = { target: { kind: 'variable', id: 'v1' }, name: 'color/bg', detach: true, uses: 5, variables: 0 };
    const asked = body(panel(), manageState({ removal }));
    expect(asked).toContain('5 uses');
    expect(asked).toMatch(/>Detach</);
    const bad = body(panel(), manageState({ drafts: { collection: { text: 'Colour', check: { name: 'Colour', error: 'notPath' } }, mode: empty, variable: empty } }));
    expect(bad).toMatch(/data-testid="manage-new-collection-note"[^>]*data-tone="error"/);
  });
  it('is the same as the Svelte body in every state of the app layer the tests build', () => {
    const removal = (target: ManageTarget, over: Partial<Removal> = {}): Removal => ({ target, name: 'x', detach: true, uses: 3, variables: 2, ...over });
    const states: [string, VariablesPanel, ManageState][] = [
      ['two collections', panel(), manageState()],
      ['the second shown', panel(), manageState({ shown: 'c2' })],
      ['none', panel([]), manageState({ shown: undefined })],
      ['a search', panel([col()], { query: 'bg' }), manageState()],
      ['cut', panel([col({ matched: 300 })]), manageState()],
      ['a variable removal', panel(), manageState({ removal: removal({ kind: 'variable', id: 'v1' }) })],
      ['a mode removal', panel(), manageState({ removal: removal({ kind: 'mode', collection: 'c1', id: 'm2' }, { detach: false }) })],
      ['a collection removal', panel(), manageState({ removal: removal({ kind: 'collection', id: 'c1' }) })],
      ['a collection rename', panel(), manageState({ rename: renaming({ kind: 'collection', id: 'c1' }, 'theme') })],
      ['a mode rename', panel(), manageState({ rename: renaming({ kind: 'mode', collection: 'c1', id: 'm1' }, 'light', { first: true }) })],
      ['a variable rename', panel(), manageState({ rename: renaming({ kind: 'variable', id: 'v2' }, 'space/4', { text: 'Space', check: { name: 'Space', error: 'notPath' }, unchanged: false }) })],
      ['a form refused', panel(), manageState({ drafts: { collection: { text: 'Colour', check: { name: 'Colour', error: 'notPath' } }, mode: { text: 'dark', check: { name: 'dark', warning: 'same' } }, variable: { text: 'a', check: { name: 'a', error: 'duplicate' } } } })],
      ['no name', panel([col({ name: '', modes: [{ id: 'm1', name: '' }, { id: 'm2', name: 'dark' }], variables: [row({ name: '' })], matched: 1 })]), manageState()],
    ];
    for (const [name, p, m] of states) expect(tokens(body(p, m)), name).toEqual(tokens(bodyS(p, m)));
  });
});

describe('the React detail pane', () => {
  it('names every button after what it acts on, and marks each row for the focus', () => {
    const out = detail(col());
    for (const label of ['Rename collection theme', 'Remove collection theme', 'Rename color/bg', 'Remove color/bg', 'Rename mode light']) expect(out).toContain(`aria-label="${label}"`);
    expect(out).toContain('data-rename="collection:c1"');
    expect(out).toContain('data-remove="variable:v1"');
    expect(out).toContain('data-entity-id="m2"');
  });
  it('puts the rename field in place of the thing it renames, and takes that thing\'s Rename button away', () => {
    const out = detail(col(), renaming({ kind: 'variable', id: 'v1' }, 'color/bg'));
    expect(out).toContain('data-testid="manage-renaming-variable"');
    expect(out).not.toContain('data-rename="variable:v1"');
    expect(out).toContain('data-rename="variable:v2"');
    expect(detail(col(), renaming({ kind: 'collection', id: 'c1' }, 'theme'))).not.toContain('data-testid="manage-title"');
    expect(detail(col(), renaming({ kind: 'mode', collection: 'other', id: 'm1' }, 'light'))).not.toContain('manage-renaming-mode');
  });
  it('says there is nothing, and when the list is cut', () => {
    expect(detail(col({ variables: [], matched: 0 }))).toContain('This collection has no variables yet. Add one above.');
    expect(detail(col({ variables: [], matched: 4 }))).toContain('No variable of this collection is listed.');
    expect(detail(col({ matched: 300 }))).toContain('Showing 2 of 300 variables.');
  });
  it('is the same as the Svelte pane for each state', () => {
    const cases: [CollectionRow, RenameState | null][] = [
      [col(), null],
      [col({ variables: [], matched: 0 }), null],
      [col({ matched: 300 }), null],
      [col({ name: '' }), null],
      [col(), renaming({ kind: 'collection', id: 'c1' }, 'theme')],
      [col(), renaming({ kind: 'mode', collection: 'c1', id: 'm2' }, 'dark')],
      [col(), renaming({ kind: 'variable', id: 'v1' }, 'color/bg', { current: 'color/page' })],
    ];
    for (const [c, r] of cases) expect(tokens(detail(c, r)), `${c.name} ${r?.target.kind}`).toEqual(tokens(detailS(c, r)));
  });
});

describe('the React dialog', () => {
  const props = { panel: panel(), manage: manageState(), events: [], dispatch: noop, opener: () => null };
  it('draws nothing while it is closed, so the page holds no manage view', () => {
    expect(renderToStaticMarkup(<ManageDialog open={false} {...props} />)).toBe('');
  });
  it('keeps the keys typed in it out of the editor, as every portaled popup does (ownskeys.test.ts)', () => {
    expect(readFileSync(new URL('./ManageDialog.tsx', import.meta.url), 'utf8')).toMatch(/<Dialog\.Popup[^>]*data-owns-keys/);
  });
  it('tells the app layer it closed, whatever closed it, and names its title, description and close button as the Svelte dialog does', () => {
    const src = readFileSync(new URL('./ManageDialog.tsx', import.meta.url), 'utf8');
    const svelte = readFileSync(new URL('../ManageDialog.svelte', import.meta.url), 'utf8');
    expect(src).toMatch(/type: 'manage', op: \{ type: 'close' \}/);
    for (const words of ['Manage variables', 'Add and remove collections, modes and variables. A removal asks first, and one undo takes it back.', 'data-testid="manage-close"', 'data-testid="manage-dialog"']) {
      expect(src, words).toContain(words);
      expect(svelte, words).toContain(words);
    }
  });
});
