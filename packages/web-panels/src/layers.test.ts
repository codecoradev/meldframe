import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type { LayerRow } from '@meldframe/web-document/layers';
import LayersPanel from './LayersPanel.svelte';

const row = (over: Partial<LayerRow>): LayerRow => ({ id: 'a', name: 'Hero', kind: 'frame', depth: 0, hasChildren: false, collapsed: false, selected: false, actor: null, ...over });
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
    const other = html([row({ actor: 'agent:a' })]);
    expect(other).toContain('data-testid="actor"');
    expect(other).toContain('title="Last changed by a"');
    expect(other).toMatch(/text-text-muted[^>]*>a</); // the name is in text on an idle row, not only a colour
    expect(other).toMatch(/bg-agent-[123]/);
    expect(html([row({ actor: 'agent:dev' })])).not.toContain('data-testid="actor"');
  });
  it('two agents do not have to share a colour, and the selected row shows the name in plain text', () => {
    const tones = new Set(['agent:a', 'agent:b', 'agent:c', 'agent:d'].map((a) => (html([row({ actor: a })]).match(/bg-agent-(\d)/) ?? [])[1]));
    expect(tones.size).toBeGreaterThan(1);
    const sel = html([row({ actor: 'agent:a', selected: true })]);
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
