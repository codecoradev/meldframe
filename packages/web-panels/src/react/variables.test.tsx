// The React copies of the Variables panel, the bind picker and the picker list, checked as variables-panel.test.ts and
// picker.test.ts check the Svelte ones: the markup of a server render, in Node. What a server render cannot show (the
// popover's portal, focus, Escape, an outside click, the position) is the job of packages/web-panels/scripts/variables-e2e.mjs
// with MELDFRAME_UI=react, which asks the Svelte and the React page the same questions in a browser.
import type { CollectionRow, Picker, PickerRow, Properties, VariableRow, VariablesPanel as Model } from '@meldframe/protocol';
import { readFileSync } from 'node:fs';
import { renderToStaticMarkup } from 'react-dom/server';
import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import PickerListSvelte from '../PickerList.svelte';
import VariablesPanelSvelte from '../VariablesPanel.svelte';
import { BindPicker, tabTarget } from './BindPicker';
import { PickerList } from './PickerList';
import { VariablesPanel } from './VariablesPanel';

// Colours of the test data, made from numbers: the literal scan of the UI code (`cargo xtask ui-literals`) reads .tsx files, tests included,
// and the swatch of a variable wants a hex value to draw.
const hexOf = (n: number) => `#${n.toString(16).padStart(6, '0')}`;
const WHITE = hexOf(0xffffff);
const BLACK = hexOf(0);
const NAVY = hexOf(0x112233);

const row = (over: Partial<VariableRow> = {}): VariableRow => ({
  id: 'bg', name: 'color/bg', type: 'color',
  values: [{ mode: 'm1', text: WHITE, resolved: WHITE }, { mode: 'm2', text: 'color/ink', alias: 'ink', resolved: BLACK }],
  usedBy: 1, binds: [{ field: 'fill', bound: false }], heading: { type: 'color', count: 1 }, ...over,
});
const modes = [{ id: 'm1', name: 'Light' }, { id: 'm2', name: 'A very long mode name that cannot fit' }, { id: 'm3', name: 'Dark' }];
const col = (over: Partial<CollectionRow> = {}): CollectionRow => ({ id: 'c', name: 'appearance', modes, active: 'm1', matched: 1, variables: [row()], ...over });
const panel = (c: CollectionRow[], over: Partial<Model> = {}): Model => ({ query: '', total: 1, matched: 1, collections: c, ...over });
const html = (p: Model, target: string | null = 'n1') => renderToStaticMarkup(<VariablesPanel panel={p} target={target} dispatch={() => {}} />);

describe('the React Variables panel as the boards draw it', () => {
  it('lists the rows in a container capped by a size token, with the search and the modes outside it', () => {
    const out = html(panel([col()]));
    expect(out).toContain('max-h-(--mf-size-variables-list)');
    expect(out).toContain('overflow-y-auto');
    const list = out.indexOf('max-h-(--mf-size-variables-list)');
    expect(out.indexOf('data-testid="var-search"')).toBeLessThan(list);
    expect(out.indexOf('data-testid="var-modes"')).toBeLessThan(list);
    expect(out.indexOf('data-testid="var-row"')).toBeGreaterThan(list);
  });
  it('names the value field after the mode name, not its id', () => {
    const out = html(panel([col()]));
    expect(out).toContain('aria-label="color/bg in Light"');
    expect(out).not.toContain('in m1');
  });
  it('says (no name) for a mode, a collection and a variable whose name is nothing, as the Layers panel does', () => {
    const empty = [{ id: 'm1', name: '' }, { id: 'm2', name: 'Dark' }];
    const out = html(panel([col({ name: '', modes: empty, variables: [row({ name: '' })] })]));
    expect(out).toContain('aria-label="(no name) in (no name)"');
    expect(out).toContain('Mode of (no name)');
    expect(out).toContain('>(no name)<');
    expect(out).not.toMatch(/>\s*<\/h3>/);
  });
  it('draws three modes with a long name truncated and titled, and says how to edit another mode', () => {
    const out = html(panel([col()]));
    expect(out.match(/role="radio"/g)).toHaveLength(3);
    expect(out).toContain('title="A very long mode name that cannot fit"');
    expect(out).toContain('truncate');
    expect(out).toContain('Switch mode to edit its values');
    expect(html(panel([col({ modes: [modes[0]!] })]))).not.toContain('Switch mode to edit');
  });
  it('draws a heading per type as a heading, not as a bare list item, and titles an alias in full', () => {
    const out = html(panel([col({ active: 'm2' })]));
    expect(out).toMatch(/<h4[^>]*>Color/);
    expect(out).toContain('title="→ color/ink"');
  });
  it('keeps the bind buttons on the row, hidden until hover or focus unless the field is bound', () => {
    const out = html(panel([col()]));
    expect(out).toContain('data-testid="var-bind"');
    expect(out).toContain('data-field="fill"');
    expect(out).toContain('group-focus-within:opacity-100');
    expect(out).toContain('aria-label="Bind color/bg to the selection"');
    expect(html(panel([col()]), null)).not.toContain('data-testid="var-bind"');
  });
  it('draws a swatch for a colour that resolves and none for a number', () => {
    expect(html(panel([col()]))).toMatch(/data-testid="var-swatch"/);
    expect(html(panel([col({ variables: [row({ type: 'number', values: [{ mode: 'm1', text: '4', resolved: '4' }] })] })]))).not.toContain('var-swatch');
  });
  it('says there is nothing, or nothing that matches, and when the search matched more than it lists', () => {
    expect(html(panel([], { total: 0, matched: 0 }))).toContain('This file has no variables yet.');
    expect(html(panel([col({ variables: [] })], { total: 2, matched: 0, query: 'zz' }))).toContain('No variable matches the search.');
    const more = html(panel([col({ matched: 4 })], { matched: 4, total: 4 }));
    expect(more).toMatch(/data-testid="var-more"[^>]*>3 more in appearance</);
    expect(more).toMatch(/data-testid="var-truncated"[^>]*>Showing 1 of 4\. Narrow the search to see the rest\./);
    expect(html(panel([col()]))).toMatch(/data-testid="var-count"[^>]*>1 of 1</);
  });
});

describe('the React Variables panel and its dialogs', () => {
  it('has a Manage button next to the count, and the Import and Export buttons before the list, in the words of the glossary', () => {
    const out = html(panel([col()]), null);
    expect(out).toMatch(/data-testid="var-manage"[^>]*>Manage</);
    expect(out).toMatch(/data-testid="var-import"[^>]*>Import variables</);
    expect(out).toMatch(/data-testid="var-export"[^>]*>Export variables</);
    expect(out.indexOf('data-testid="var-export"')).toBeLessThan(out.indexOf('max-h-(--mf-size-variables-list)'));
    expect(out).not.toMatch(/>(Import|Export) tokens?</i);
  });
  it('draws no dialog of its own: they are the host\'s until their slices', () => {
    const out = html(panel([col()]));
    expect(out).not.toContain('manage-dialog');
    expect(out).not.toContain('export-vars-dialog');
    expect(out).not.toContain('import-vars-dialog');
  });
  it('is not a section of its own: the Section around it is the one landmark with the name', () => {
    expect(html(panel([], { total: 0, matched: 0 }))).not.toMatch(/<section[^>]*aria-label="Variables"/);
  });
});

const prow = (over: Partial<PickerRow> = {}): PickerRow => ({ id: 'v1', name: 'color/bg', collection: 'Theme', type: 'color', resolved: NAVY, bound: false, ...over });
const picker = (over: Partial<Picker> = {}): Picker => ({ id: 'n', field: 'fill', type: 'color', query: '', total: 2, rows: [prow(), prow({ id: 'v2', name: 'color/fg', resolved: WHITE, bound: true })], ...over });
const list = (p: Picker) => renderToStaticMarkup(<PickerList picker={p} dispatch={() => {}} />);

describe('the React content of the bind picker as the Inspector board draws it', () => {
  it('has a search holding the query and a list named for the field', () => {
    const out = list(picker({ query: 'bg' }));
    expect(out).toContain('aria-label="Search variables"');
    expect(out).toMatch(/data-testid="picker-search"/);
    expect(out).toContain('aria-label="Variables for Fill"');
  });
  it('has a row for each variable with its name, its value and, for a colour, a swatch of the value', () => {
    const out = list(picker());
    expect(out.match(/data-testid="picker-row"/g)).toHaveLength(2);
    expect(out).toContain('color/bg');
    expect(out).toContain(WHITE);
    expect(out).toContain(`background:${NAVY}`);
    expect(out).toContain('title="color/bg · Theme"');
  });
  it('shows the row the field is bound to as pressed, and the others as not', () => {
    const out = list(picker());
    expect(out.match(/aria-pressed="true"/g)).toHaveLength(1);
    expect(out.match(/aria-pressed="false"/g)).toHaveLength(1);
    expect(out).toMatch(/aria-pressed="true"[^>]*data-id="v2"|data-id="v2"[^>]*aria-pressed="true"/);
  });
  it('draws no swatch for a number', () => {
    const out = list(picker({ field: 'opacity', type: 'number', rows: [prow({ type: 'number', resolved: '0.5' })] }));
    expect(out).not.toContain('background:');
    expect(out).toContain('aria-label="Variables for Opacity"');
    expect(out).toContain('Only number variables are shown for Opacity.');
  });
  it('says there are no variables of the type, or none that match the search, and only then', () => {
    expect(list(picker({ total: 0, rows: [] }))).toMatch(/data-testid="picker-empty"[^>]*>This file has no color variables\./);
    expect(list(picker({ total: 3, rows: [], query: 'zz' }))).toMatch(/data-testid="picker-empty"[^>]*>No variable matches the search\./);
    expect(list(picker())).not.toContain('picker-empty');
  });
});

describe('the React bind picker', () => {
  const props = { id: 'n', name: 'Card' } as Properties;
  const closed = renderToStaticMarkup(<BindPicker properties={props} field="fill" picker={null} dispatch={() => {}} />);
  it('draws the button of its field and nothing else while the app state has no picker for it', () => {
    expect(closed).toContain('aria-label="Bind Fill to a variable"');
    expect(closed).toContain('data-testid="prop-fill-bind"');
    expect(closed).toContain('aria-haspopup="dialog"');
    expect(closed).toContain('aria-expanded="false"');
    expect(closed).not.toContain('data-testid="picker"');
  });
  it('names the button of each field', () => {
    for (const [field, name] of [['opacity', 'Opacity'], ['radius', 'Radius']] as const) {
      expect(renderToStaticMarkup(<BindPicker properties={props} field={field} picker={null} dispatch={() => {}} />)).toContain(`aria-label="Bind ${name} to a variable"`);
    }
  });
  it('keeps the keys typed in the popover out of the editor, as every portaled popup does (ownskeys.test.ts)', () => {
    expect(readFileSync(new URL('./BindPicker.tsx', import.meta.url), 'utf8')).toMatch(/<Base\.Popup[^>]*data-owns-keys/);
  });
});

describe('where Tab goes in the bind picker (plain stubs: the Node tests have no DOM)', () => {
  const [search, a, b] = ['search', 'row-a', 'row-b'];
  const stops = [search, a, b];
  it('sends Tab from the last stop to the first, and leaves every other Tab to the browser', () => {
    expect(tabTarget(stops, b, false)).toBe(search);
    expect(tabTarget(stops, search, false)).toBeNull();
    expect(tabTarget(stops, a, false)).toBeNull();
  });
  it('sends Shift+Tab from the first stop, or from the popup itself, to the last', () => {
    expect(tabTarget(stops, search, true)).toBe(b);
    expect(tabTarget(stops, 'popup', true, 'popup')).toBe(b);
    expect(tabTarget(stops, a, true)).toBeNull();
    expect(tabTarget(stops, 'popup', true)).toBeNull(); // no root given: the popup is not a stop
  });
  it('does nothing when there is no stop, and wraps a single stop onto itself', () => {
    expect(tabTarget([], search, false)).toBeNull();
    expect(tabTarget([], search, true, search)).toBeNull();
    expect(tabTarget([search], search, false)).toBe(search);
    expect(tabTarget([search], search, true)).toBe(search);
  });
});

// The two copies draw the same elements with the same classes in the same order (a class that is not here is a visible
// difference). What a class cannot say (the real pixels) is checked in a browser by pixels-e2e.mjs, with a tolerance of 0; the position of the
// popover and the keys of the picker are checked by variables-e2e.mjs.
describe('the React copies against the Svelte ones', () => {
  const svelte = (component: unknown, props: Record<string, unknown>) => render(component as never, { props: props as never }).body.replace(/<!--.*?-->/g, '');
  const classes = (out: string) => [...out.matchAll(/class="([^"]*)"/g)].map((m) => m[1]!.split(/\s+/).filter(Boolean).sort().join(' '));
  const attrs = (out: string, name: string) => [...out.matchAll(new RegExp(`${name}="([^"]*)"`, 'g'))].map((m) => m[1]!);
  const names = ['id', 'role', 'data-testid', 'data-id', 'data-field', 'aria-label', 'aria-pressed', 'aria-checked', 'title'];
  it('the Variables panel has the same classes, roles, names, test ids and titles', () => {
    const many = panel([col(), col({ id: 'd', name: 'spacing', modes: [modes[0]!], active: 'm1', variables: [row({ id: 'gap', name: 'space/gap', type: 'number', values: [{ mode: 'm1', text: '4', resolved: '4' }], binds: [{ field: 'radius', bound: true }] })] })], { total: 2, matched: 3 });
    const a = svelte(VariablesPanelSvelte, { panel: many, target: 'n1', dispatch: () => {} });
    const b = html(many);
    // The Svelte copy wraps the three buttons that open a dialog in `span.contents` (for `bind:this`, to give the focus back); the
    // React copy has no use for the wrapper, and `display: contents` draws nothing, so they are the one difference.
    expect(classes(b).filter((c) => c !== '')).toEqual(classes(a).filter((c) => c !== '' && c !== 'contents'));
    for (const name of names) expect(attrs(b, name), name).toEqual(attrs(a, name));
  });
  it('the picker list has the same classes, roles, names, test ids and titles', () => {
    const a = svelte(PickerListSvelte, { picker: picker(), dispatch: () => {} });
    const b = list(picker());
    expect(classes(b).map((c) => c.trim())).toEqual(classes(a).map((c) => c.trim()));
    for (const name of names) expect(attrs(b, name), name).toEqual(attrs(a, name));
  });
});

describe('the React Variables panel opens its dialogs by the state of the app layer (finding T11)', () => {
  const src = readFileSync(new URL('./VariablesPanel.tsx', import.meta.url), 'utf8');
  it('sends the same open actions as the Svelte copy and has no open callback or flag', () => {
    expect(src).toContain("dispatch({ type: 'manage', op: { type: 'open' } })");
    expect(src).toContain("dispatch({ type: 'openVariablesImport' })");
    expect(src).not.toMatch(/onManage|onImport|useState|Dialog/);
  });
});
