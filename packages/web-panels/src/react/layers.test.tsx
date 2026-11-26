// The React copy of the Layers panel, checked as layers.test.ts checks the Svelte one: the markup of a server render, in Node.
// Clicks, the selection and the fold are in the browser scripts (layers-e2e.mjs and editor-e2e.mjs, both with MELDFRAME_UI=react).
import type { LayerRow } from '@meldframe/web-document/layers';
import { renderToStaticMarkup } from 'react-dom/server';
import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import LayersPanelSvelte from '../LayersPanel.svelte';
import { LayersPanel } from './LayersPanel';

const row = (over: Partial<LayerRow>): LayerRow => ({ id: 'a', name: 'Hero', kind: 'frame', depth: 0, hasChildren: false, collapsed: false, selected: false, actor: null, ...over });
const html = (rows: LayerRow[], self = 'agent:dev') => renderToStaticMarkup(<LayersPanel rows={rows} self={self} onSelect={() => {}} onToggle={() => {}} />);

describe('the React layers panel, as the Svelte one and the Panels and EditorLight boards draw it', () => {
  it('keeps the test ids of a row', () => {
    const out = html([row({ id: 'n1', selected: true })]);
    expect(out).toContain('data-testid="layers"');
    expect(out).toContain('data-testid="layer"');
    expect(out).toContain('data-id="n1"');
    expect(out).toContain('data-selected="true"');
    expect(html([row({})])).toContain('data-selected="false"');
  });
  it('has a Layers title and rows 28 high, with the selected row on the selection fill and text', () => {
    const out = html([row({ id: 'a' }), row({ id: 'b', selected: true })]);
    expect(out).toMatch(/>Layers</);
    expect(out).toContain('aria-labelledby="layers-title"');
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
    expect(out).toContain('padding-left:calc(var(--mf-space-2) + 2 * var(--mf-space-5))');
    expect(out).toContain('aria-label="Expand Hero"');
    expect(out).toContain('aria-expanded="false"');
    expect(html([row({ hasChildren: true })])).toContain('aria-label="Collapse Hero"');
  });
  it('shows another actor as a dot in its colour with the name in text for readers, and none for the page itself', () => {
    const other = html([row({ actor: 'agent:a' })]);
    expect(other).toContain('data-testid="actor"');
    expect(other).toContain('title="Last changed by a"');
    expect(other).toMatch(/text-text-muted[^>]*>a</); // the name is in text on an idle row, not only a colour
    expect(other).toMatch(/bg-agent-[123]/);
    expect(html([row({ actor: 'agent:dev' })])).not.toContain('data-testid="actor"');
    expect(html([row({ actor: 'person:ana' })])).toMatch(/rounded-pill bg-person/);
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

// The two copies draw the same elements with the same classes in the same order (a class that is not here is a visible
// difference). What a class cannot say (the real pixels) is checked in a browser by pixels-e2e.mjs, which compares the two copies'
// images with a tolerance of 0; layers-e2e.mjs checks behaviour only.
describe('the React layers panel against the Svelte one', () => {
  const rows = [
    row({ id: 'f', name: 'Frame', depth: 0, hasChildren: true }),
    row({ id: 't', name: 'Title', kind: 'text', depth: 1, selected: true, actor: 'agent:a' }),
    row({ id: 'c', name: 'Card', kind: 'ellipse', depth: 2, hasChildren: true, collapsed: true, actor: 'person:ana' }),
  ];
  const svelte = render(LayersPanelSvelte as never, { props: { rows, self: 'agent:dev', onSelect: () => {}, onToggle: () => {} } as never }).body.replace(/<!--.*?-->/g, '');
  const react = html(rows);
  const classes = (out: string) => [...out.matchAll(/class="([^"]*)"/g)].map((m) => m[1]!.split(/\s+/).filter(Boolean).sort().join(' '));
  const attrs = (out: string, name: string) => [...out.matchAll(new RegExp(`${name}="([^"]*)"`, 'g'))].map((m) => m[1]!);
  it('has the same classes, roles, names, test ids and titles', () => {
    expect(classes(react)).toEqual(classes(svelte));
    for (const name of ['id', 'data-testid', 'data-id', 'data-selected', 'aria-label', 'aria-labelledby', 'aria-expanded', 'aria-current', 'title']) {
      expect(attrs(react, name), name).toEqual(attrs(svelte, name));
    }
  });
});
