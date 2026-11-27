import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import PropertiesPanel from './PropertiesPanel.svelte';
import VariablesPanel from './VariablesPanel.svelte';

// The Section around a panel is the one landmark with its name; the panel inside it must not add a second one.
describe('one landmark per section', () => {
  it('the Variables panel and the Properties panel are not sections of their own', () => {
    const variables = render(VariablesPanel as never, { props: { panel: { collections: [], matched: 0, total: 0, query: '' }, target: null, dispatch: () => {} } as never }).body;
    const properties = render(PropertiesPanel as never, { props: { properties: null, onProperty: () => {}, onBox: () => {} } as never }).body;
    expect(variables).toContain('data-testid="variables"');
    expect(properties).toContain('data-testid="properties"');
    for (const out of [variables, properties]) expect(out).not.toMatch(/<section[^>]*aria-label="(Variables|Properties)"/);
  });
});
