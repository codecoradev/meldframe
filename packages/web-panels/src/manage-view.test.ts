import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type { CollectionRow, VariableRow, VariablesPanel } from '@meldframe/protocol';
import ManageBody from './ManageBody.svelte';
import RemoveConfirm from './RemoveConfirm.svelte';
import VariablesPanelView from './VariablesPanel.svelte';
import { removalPlan } from './manage';

const row = (over: Partial<VariableRow> = {}): VariableRow => ({ id: 'v1', name: 'color/bg', type: 'color', values: [], usedBy: 2, binds: [], ...over });
const modes = [{ id: 'm1', name: 'light' }, { id: 'm2', name: 'dark' }];
const col = (over: Partial<CollectionRow> = {}): CollectionRow => ({ id: 'c1', name: 'theme', modes, active: 'm1', matched: 2, variables: [row(), row({ id: 'v2', name: 'space/4', type: 'number', usedBy: 0 })], ...over });
const panel = (collections: CollectionRow[] = [col(), col({ id: 'c2', name: 'brand', modes: [{ id: 'm9', name: 'default' }], matched: 0, variables: [] })], over: Partial<VariablesPanel> = {}): VariablesPanel => ({ query: '', total: 2, matched: 2, collections, ...over });
const body = (p: VariablesPanel) => render(ManageBody as never, { props: { panel: p, events: [], dispatch: () => {} } as never }).body;

describe('the manage view', () => {
  it('lists the collections with their variables and modes, and opens on the first', () => {
    const out = body(panel());
    expect(out.match(/data-testid="manage-collection"/g)).toHaveLength(2);
    expect(out).toContain('2 variables');
    expect(out).toContain('0 variables');
    expect(out.match(/data-testid="manage-var"/g)).toHaveLength(2);
    expect(out.match(/data-testid="manage-mode"/g)).toHaveLength(2);
    expect(out).toMatch(/aria-current="true"[^>]*data-id="c1"|data-id="c1"[^>]*aria-current="true"/);
  });
  it('shows the type and the uses of each variable and the uses of the collection', () => {
    const out = body(panel());
    expect(out).toContain('Color');
    expect(out).toContain('Number');
    expect(out).toMatch(/2 uses/);
    expect(out).toContain('scope="col"');
    expect(out).toContain('scope="row"');
  });
  it('marks the first mode as the default one and offers no removal for it', () => {
    const out = body(panel());
    expect(out).toContain('default');
    expect(out).toContain('aria-label="Remove mode dark"');
    expect(out).not.toContain('aria-label="Remove mode light"');
  });
  it('has a labelled field for each of the three things that can be added, with Add off while it is empty', () => {
    const out = body(panel());
    for (const label of ['New collection name', 'New mode name', 'New variable name']) expect(out).toContain(`aria-label="${label}"`);
    expect(out.match(/data-testid="manage-new-[a-z]+-add"[^>]*disabled|disabled[^>]*data-testid="manage-new-[a-z]+-add"/g)).toHaveLength(3);
    expect(out).toContain('lowercase letters, digits and single hyphens');
  });
  it('asks the type of a new variable and where a new mode takes its values from', () => {
    const out = body(panel());
    expect(out.match(/role="radio"/g)).toHaveLength(2);
    expect(out).toContain('aria-label="Type of the new variable"');
    expect(out).toContain('aria-label="Copy the values of the new mode from"');
    expect(out).toMatch(/<option[^>]*value="m1"[^>]*>light/);
  });
  it('draws names as text: markup in a name is escaped and direction characters are left out', () => {
    const out = body(panel([col({ name: 'br‮and', variables: [row({ name: '<img src=x onerror=alert(1)>' })], matched: 1, modes: [{ id: 'm1', name: 'li⁦ght' }, { id: 'm2', name: '<b>dark</b>' }] })]));
    expect(out).not.toContain('<img src=x');
    expect(out).toContain('&lt;img src=x');
    expect(out).not.toContain('<b>dark');
    expect(out).toContain('&lt;b>dark');
    expect(out).not.toMatch(/[‮⁦]/);
    expect(out).toContain('brand');
  });
  it('says when the list is cut or narrowed by a search, so the counts are known to be partial', () => {
    expect(body(panel())).not.toContain('data-testid="manage-partial"');
    expect(body(panel([col({ matched: 300 })]))).toContain('data-testid="manage-partial"');
    expect(body(panel([col()], { query: 'bg' }))).toContain('data-testid="manage-partial"');
  });
  it('says what to do in a file with no collection, and has no detail pane then', () => {
    const out = body(panel([]));
    expect(out).toMatch(/no collections yet/i);
    expect(out).not.toContain('data-testid="manage-var"');
    expect(out).toContain('aria-label="New collection name"');
  });
  it('keeps the table in a labelled region and names every removal button after what it removes', () => {
    const out = body(panel());
    expect(out).toContain('aria-label="Remove color/bg"');
    expect(out).toContain('aria-label="Remove collection theme"');
  });
  it('never uses markup from a name: no raw html in the view', () => {
    for (const f of ['ManageBody', 'CollectionList', 'CollectionDetail', 'NameForm', 'RemoveConfirm', 'ManageDialog']) {
      expect(readSource(f), f).not.toContain('{@html');
    }
  });
});

import { readFileSync } from 'node:fs';
const readSource = (name: string) => readFileSync(new URL(`./${name}.svelte`, import.meta.url), 'utf8');

describe('the confirmation of a removal', () => {
  const confirm = (plan = removalPlan({ kind: 'variable', row: row({ usedBy: 3 }) })) => render(RemoveConfirm as never, { props: { plan, onConfirm: () => {}, onCancel: () => {} } as never }).body;
  it('says what will happen and offers Detach or Cancel, with Cancel first in the order of the focus', () => {
    const out = confirm();
    expect(out).toContain('3 uses');
    expect(out).toMatch(/>Detach</);
    expect(out).toMatch(/>Cancel</);
    expect(out).toContain('role="alertdialog"');
    expect(out).toContain('aria-label="Confirm the removal"');
  });
  it('offers Remove when nothing is let go', () => {
    expect(confirm(removalPlan({ kind: 'variable', row: row({ usedBy: 0 }) }))).toMatch(/>Remove</);
  });
  it('draws the Detach button as destructive', () => {
    expect(confirm()).toContain('bg-danger-strong');
  });
});

describe('the Variables panel and the manage view', () => {
  const html = (p: VariablesPanel) => render(VariablesPanelView as never, { props: { panel: p, target: null, dispatch: () => {}, events: [] } as never }).body;
  it('has a Manage button next to the count that opens the view', () => {
    const out = html(panel());
    expect(out).toContain('data-testid="var-manage"');
    expect(out).toMatch(/>Manage</);
  });
});
