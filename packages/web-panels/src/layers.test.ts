import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type { LayerRow } from '@meldframe/protocol';
import { actorView } from '@meldframe/web-document/fixtures';
import LayersPanel from './LayersPanel.svelte';
import { KIND_ICON, changedBy } from './layers';

const row = (over: Partial<LayerRow>): LayerRow => ({ id: 'a', name: 'Hero', kind: 'frame', depth: 0, hasChildren: false, collapsed: false, selected: false, actor: null, ...over });
const who = (actor: string, tone?: Parameters<typeof actorView>[1]) => ({ actor: actorView(actor, tone) });
const html = (rows: LayerRow[], self = 'agent:dev') =>
  render(LayersPanel as never, { props: { rows, self, onSelect: () => {}, onToggle: () => {} } as never }).body;

describe('the layers panel as the Panels and EditorLight boards draw it', () => {
  it('keeps the test ids of a row', () => {
    const out = html([row({ id: 'n1', selected: true })]);
    expect(out).toContain('data-testid="layers"');
    expect(out).toContain('data-testid="layer"');
    expect(out).toContain('data-id="n1"');
    expect(out).toContain('data-selected="true"');
  });
  it('has a Layers title and rows 28 high, with the selected row on the selection fill and text', () => {
    const out = html([row({ id: 'a' }), row({ id: 'b', selected: true })]);
    expect(out).toMatch(/>Layers</);
    expect(out.match(/h-\(--mf-size-control-md\)/g)).toHaveLength(2);
    expect(out.match(/bg-selection-bg text-selection-text/g)).toHaveLength(1);
    expect(out).toMatch(/<button[^>]*aria-current="true"/);
    expect(out).not.toMatch(/<li[^>]*aria-current/);
  });
  it('draws the kind as an icon and the name as the text, not the kind word', () => {
    const out = html([row({ kind: 'rect', name: 'Leaf' })]);
    expect(out).toContain('<svg');
    expect(out).toContain('Leaf');
    expect(out).not.toMatch(/>rect</);
  });
  it('indents by depth on the 4 px steps and folds with a named chevron', () => {
    const out = html([row({ depth: 2, hasChildren: true, collapsed: true })]);
    expect(out).toContain('padding-left: calc(var(--mf-space-2) + 2 * var(--mf-space-5))');
    expect(out).toContain('aria-label="Expand Hero"');
    expect(out).toContain('aria-expanded="false"');
  });
  it('shows another actor as a dot in its colour with the name in text for readers, and none for the page itself', () => {
    const other = html([row(who('agent:a'))]);
    expect(other).toContain('data-testid="actor"');
    expect(other).toContain('title="Last changed by a"');
    expect(other).toMatch(/text-text-muted[^>]*>a</); // the name is in text on an idle row, not only a colour
    expect(other).toMatch(/bg-agent-[123]/);
    expect(html([row(who('agent:dev'))])).not.toContain('data-testid="actor"');
  });
  it('draws the tone the app layer gave the actor, a person in the person colour, and works nothing out from the id', () => {
    const dot = (actor: string, tone: Parameters<typeof actorView>[1]) => (html([row(who(actor, tone))]).match(/rounded-pill (bg-[a-z0-9-]+)/) ?? [])[1];
    expect([dot('agent:a', 'agent1'), dot('agent:a', 'agent2'), dot('agent:a', 'agent3'), dot('person:ana', 'person')]).toEqual(['bg-agent-1', 'bg-agent-2', 'bg-agent-3', 'bg-person']);
    // Whatever the characters of the id, the colour is the one the view carries (a non-BMP id gave two colours before).
    expect(dot('agent:\u{1F300}', 'agent3')).toBe('bg-agent-3');
    expect(html([row(who('agent:\u{1F300}', 'agent3'))])).toContain('data-tone="agent3"');
  });
  it('draws the name of the app layer as it is, and a node with no name left in words', () => {
    // The app layer made every name safe to show; this panel draws it and cleans nothing.
    expect(html([row({ name: 'a b' })])).toContain('a b');
    const out = html([row({ name: '', hasChildren: true })]);
    expect(out).toContain('(no name)');
    expect(out).toContain('aria-label="Collapse (no name)"');
  });
  it('the selected row shows the name in plain text', () => {
    const sel = html([row({ ...who('agent:a'), selected: true })]);
    expect(sel).toMatch(/text-selection-text[^>]*>a</);
    expect(sel).not.toContain('sr-only');
    expect(sel).toMatch(/aria-current="true"/);
  });
  it('has no landmark of its own to nest in the card, and every button on a row is at least 24 px', () => {
    const out = html([row({ hasChildren: true })]);
    expect(out).not.toContain('<section');
    expect(out).toContain('min-w-(--mf-size-control-sm)');
    expect(out).not.toContain('text-icon-muted'); // icon-muted misses 3:1 on a hovered or selected row
    expect(out.match(/min-h-\(--mf-size-control-sm\)/g)).toHaveLength(1);
  });
  it('says so when there is nothing, in a readable colour', () => {
    const out = html([]);
    expect(out).toContain('Nothing here yet.');
    expect(out).not.toContain('text-text-faint');
  });
});

describe('changedBy, which both copies of the panel call', () => {
  it('names another actor and says nothing for the page itself or a node nobody changed', () => {
    expect(changedBy(row(who('agent:a')), 'agent:dev')?.name).toBe('a');
    expect(changedBy(row(who('agent:dev')), 'agent:dev')).toBeNull();
    expect(changedBy(row({ actor: null }), 'agent:dev')).toBeNull();
  });
  it('maps the tone of the app layer to a class and works nothing out from the id', () => {
    const dot = (a: string, tone: Parameters<typeof actorView>[1]) => changedBy(row(who(a, tone)), 'agent:dev')?.dot;
    expect([dot('person:ana', 'person'), dot('agent:a', 'agent1'), dot('agent:a', 'agent2'), dot('agent:a', 'agent3')]).toEqual(['bg-person', 'bg-agent-1', 'bg-agent-2', 'bg-agent-3']);
    expect(dot('agent:\u{1F300}', 'agent3')).toBe('bg-agent-3');
  });
  it('knows an icon for every kind of node', () => {
    expect(Object.keys(KIND_ICON).sort()).toEqual(['ellipse', 'frame', 'rect', 'shader', 'text']);
  });
});
