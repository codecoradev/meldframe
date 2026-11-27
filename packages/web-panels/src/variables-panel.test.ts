import { readFileSync } from 'node:fs';
import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type { CollectionRow, VariableRow, VariablesPanel } from '@meldframe/protocol';
import VariablesPanelView from './VariablesPanel.svelte';

const row = (over: Partial<VariableRow> = {}): VariableRow => ({
  id: 'bg', name: 'color/bg', type: 'color',
  values: [{ mode: 'm1', text: '#ffffff', resolved: '#ffffff' }, { mode: 'm2', text: 'color/ink', alias: 'ink', resolved: '#000000' }],
  usedBy: 1, binds: [{ field: 'fill', bound: false }], heading: { type: 'color', count: 1 }, ...over,
});
const modes = [{ id: 'm1', name: 'Light' }, { id: 'm2', name: 'A very long mode name that cannot fit' }, { id: 'm3', name: 'Dark' }];
const col = (over: Partial<CollectionRow> = {}): CollectionRow => ({ id: 'c', name: 'appearance', modes, active: 'm1', matched: 1, variables: [row()], ...over });
const panel = (c: CollectionRow[]): VariablesPanel => ({ query: '', total: 1, matched: 1, collections: c });
const html = (p: VariablesPanel, target: string | null = 'n1') => render(VariablesPanelView as never, { props: { panel: p, target, dispatch: () => {} } as never }).body;

describe('the Variables section as the boards draw it', () => {
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
    expect(out).toContain('group-focus-within:opacity-100');
    expect(html(panel([col()]), null)).not.toContain('data-testid="var-bind"');
  });
});

describe('the Variables panel and the manage view', () => {
  it('has a Manage button next to the count that opens the view', () => {
    const out = html(panel([col()]), null);
    expect(out).toContain('data-testid="var-manage"');
    expect(out).toMatch(/>Manage</);
  });
});

describe('the Import variables and Export variables buttons', () => {
  const out = html(panel([col()]));
  it('are in the header of the section, before the list, named with the glossary words', () => {
    expect(out).toMatch(/data-testid="var-import"[^>]*>(?:<!---->)?Import variables</);
    expect(out).toMatch(/data-testid="var-export"[^>]*>(?:<!---->)?Export variables</);
    expect(out.indexOf('data-testid="var-export"')).toBeLessThan(out.indexOf('max-h-(--mf-size-variables-list)'));
    expect(out).not.toMatch(/>(Import|Export) tokens?</i);
  });
  it('are there with no io given (a section drawn alone), and the panel draws no dialog', () => {
    expect(out).not.toContain('data-testid="export-vars-dialog"');
    expect(out).not.toContain('data-testid="import-vars-dialog"');
  });
});

describe('the dialogs of the Variables section are open by the state of the app layer (finding T11)', () => {
  // The panel holds no flag and draws no dialog: Manage and Import variables are open while `manage.open` and `variablesIo.importOpen` say so,
  // Export variables while the app layer holds its text, and the editor page draws them beside the Export dialog (apps/web/src/dialogs-host.test.ts),
  // so that they do not go when the panel does. The three buttons send the actions that open them. The clicks are checked in a browser (variables-e2e.mjs).
  const src = readFileSync(new URL('./VariablesPanel.svelte', import.meta.url), 'utf8');
  it('sends the open actions and keeps no open flag of its own', () => {
    expect(src).toContain("dispatch({ type: 'manage', op: { type: 'open' } })");
    expect(src).toContain("dispatch({ type: 'openVariablesImport' })");
    expect(src).toContain("dispatch({ type: 'openVariablesExport' })");
    expect(src).not.toMatch(/\$state\((false|true)\)/);
    expect(src).not.toContain('onOpenChange');
  });
  it('draws no dialog: none is imported, so none can go with the panel', () => {
    expect(src).not.toMatch(/Dialog|ManageView|ImportView|ExportView/);
  });
});
