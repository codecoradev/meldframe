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
