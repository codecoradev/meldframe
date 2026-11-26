// The React Properties panel, checked as properties.test.ts checks the Svelte one: the markup of a server render, in Node. Typing,
// clicks, the focus and the pixels are the browser scripts' (properties-e2e.mjs, text-style-e2e.mjs, border-remove-e2e.mjs,
// ellipse-e2e.mjs and editor-e2e.mjs with MELDFRAME_UI=react, and pixels-e2e.mjs, which compares the two copies' images).
import type { AssetsState, Properties } from '@meldframe/protocol';
import TestHost from '@meldframe/web-chrome/TestHost.svelte';
import { renderToStaticMarkup } from 'react-dom/server';
import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import PropertiesPanelSvelte from '../PropertiesPanel.svelte';
import { PropertiesPanel } from './PropertiesPanel';
import { viewOf } from './compare';
import { FRAME, LINE_B, RECT, TEXT, binding, gradient, properties, shadow } from './fixture';

const html = (p: Properties | null, selected = p ? 1 : 0, over: Partial<Parameters<typeof PropertiesPanel>[0]> = {}) =>
  renderToStaticMarkup(<PropertiesPanel properties={p} selected={selected} dispatch={() => {}} {...over} />);

describe('the React properties panel as the Properties and Inspector boards draw it', () => {
  it('has the sections Position, Size, Appearance and Fill in that order, then Border and Shadow', () => {
    const out = html(properties());
    const at = ['Position', 'Size', 'Layout', 'Appearance', 'Fill', 'Border', 'Shadow'].map((t) => out.indexOf(`aria-label="${t}"`));
    expect(at.every((i) => i >= 0)).toBe(true);
    expect([...at].sort((a, b) => a - b)).toEqual(at);
  });
  it('keeps the test ids of the fields and shows opacity as a percentage', () => {
    const out = html(properties());
    for (const id of ['prop-name', 'prop-x', 'prop-y', 'prop-w', 'prop-h', 'prop-fill', 'prop-radius', 'prop-opacity', 'prop-clip']) expect(out).toContain(`data-testid="${id}"`);
    expect(out).toMatch(/data-testid="prop-opacity"[^>]*value="50"|value="50"[^>]*data-testid="prop-opacity"/);
  });
  it('rounds a position and a size to two decimals, and draws the name as the app layer gave it', () => {
    const out = html(properties({ name: 'a b', x: 10.126, w: 300.5 }));
    expect(out).toMatch(/data-testid="prop-x"[^>]*value="10.13"|value="10.13"[^>]*data-testid="prop-x"/);
    expect(out).toMatch(/data-testid="prop-w"[^>]*value="300.5"|value="300.5"[^>]*data-testid="prop-w"/);
    expect(out).toMatch(/data-testid="prop-name"[^>]*value="a b"|value="a b"[^>]*data-testid="prop-name"/);
  });
  it('leaves x and y editable, with no note, while the position is the node\'s own', () => {
    const out = html(properties());
    expect(out).not.toMatch(/data-testid="prop-x"[^>]*disabled|disabled[^>]*data-testid="prop-x"/);
    expect(out).not.toContain('prop-position-note');
  });
  it('disables x and y and says why, tied to both fields, while the position comes from the auto layout of the parent', () => {
    const out = html(properties({ positionLocked: true, inLayout: true }));
    for (const id of ['prop-x', 'prop-y']) {
      const input = out.match(new RegExp(`<input[^>]*data-testid="${id}"[^>]*>`))![0];
      expect(input).toContain('disabled');
      expect(input).toContain('aria-describedby="prop-position-note"');
    }
    expect(out).toMatch(/id="prop-position-note"[^>]*>[^<]*auto layout/);
    for (const id of ['prop-w', 'prop-h']) expect(out.match(new RegExp(`<input[^>]*data-testid="${id}"[^>]*>`))![0]).not.toContain('disabled');
  });
  it('offers the fields of the kind: no corners on text, a font size instead, and only a frame clips', () => {
    const text = html(properties({ kind: 'text', sections: TEXT }));
    expect(text).toContain('data-testid="prop-fontSize"');
    expect(text).not.toContain('data-testid="prop-radius"');
    expect(text).not.toContain('data-testid="prop-clip"');
    expect(html(properties({ kind: 'rect', sections: RECT }))).not.toContain('data-testid="prop-clip"');
    expect(html(properties({ kind: 'rect', sections: RECT }))).not.toContain('aria-label="Layout"');
  });
  it('offers a weight and an alignment on a text, labelled, and on no other kind', () => {
    const text = html(properties({ kind: 'text', sections: TEXT, fontWeight: 700, textAlign: 'center' }));
    expect(text).toContain('data-testid="prop-fontWeight"');
    expect(text).toMatch(/<label for="prop-fontWeight"[^>]*>Font weight<\/label>/);
    expect(text).toContain('data-testid="prop-textAlign"');
    for (const kind of ['frame', 'rect', 'ellipse'] as const) {
      const out = html(properties({ kind }));
      expect(out, kind).not.toContain('data-testid="prop-fontWeight"');
      expect(out, kind).not.toContain('data-testid="prop-textAlign"');
    }
  });
  it('shows the Layout section of a frame and the Margin section where the app layer says the node has a placement', () => {
    expect(html(properties())).toContain('data-testid="prop-layout"');
    expect(html(properties())).not.toContain('data-testid="prop-margin-top"');
    const placed = html(properties({ kind: 'rect', sections: { ...RECT, placement: true }, inLayout: true }));
    for (const s of ['top', 'right', 'bottom', 'left']) expect(placed).toContain(`data-testid="prop-margin-${s}"`);
    expect(placed).toContain('data-testid="prop-absolute"');
  });
  it('shows a bound fill as a chip with the variable name and a way to detach it', () => {
    const out = html(properties({ bound: ['fill'], bindings: [binding('fill', { name: 'color/brand' })] }));
    expect(out).toContain('data-testid="prop-fill-bound"');
    expect(out).toContain('color/brand');
    expect(out).toContain('bg-chip-bg');
    expect(out).toContain('aria-label="Detach color/brand"');
  });
  it('shows a bound opacity and a bound corner radius as a full-width chip with a detach button each', () => {
    const out = html(properties({ bound: ['opacity', 'radius'], bindings: [binding('opacity'), binding('radius')] }));
    for (const f of ['opacity', 'radius']) {
      expect(out).toContain(`data-testid="prop-${f}-bound"`);
      expect(out).toContain(`data-testid="prop-${f}-detach"`);
    }
    expect(out).toContain('col-span-2');
    expect(out).toContain('aria-label="Detach space/md"');
    expect(out).toMatch(/data-testid="prop-opacity"[^>]*>50 %</);
  });
  it('shows a binding whose variable is gone as a red named chip that says so', () => {
    const out = html(properties({ bound: ['fill'], bindings: [binding('fill', { name: 'ghost', missing: true })] }));
    expect(out).toContain('bg-danger-bg');
    expect(out).toContain('ghost (removed)');
  });
  it('has a bind button on each field that takes a variable, and none on a field that is bound already', () => {
    const out = html(properties());
    for (const f of ['fill', 'opacity', 'radius']) expect(out).toContain(`data-testid="prop-${f}-bind"`);
    for (const f of ['fill', 'opacity', 'radius'] as const) {
      const bound = html(properties({ bound: [f], bindings: [binding(f)] }));
      expect(bound, f).not.toContain(`data-testid="prop-${f}-bind"`);
      for (const other of ['fill', 'opacity', 'radius'].filter((x) => x !== f)) expect(bound, `${f} bound, ${other}`).toContain(`data-testid="prop-${other}-bind"`);
    }
    expect(html(properties({ kind: 'text', sections: TEXT }))).not.toContain('data-testid="prop-radius-bind"');
    expect(html(properties({ kind: 'text', sections: TEXT }))).toContain('data-testid="prop-opacity-bind"');
  });
  it('is one group named Properties with a rule above it, and its sections are the landmarks inside', () => {
    const out = html(properties());
    expect(out).toMatch(/<div[^>]*role="group"[^>]*aria-label="Properties"/);
    expect(out).toContain('border-t border-hairline');
    expect(out.match(/<section/g)?.length).toBeGreaterThanOrEqual(4);
    expect(out.match(/aria-label="Properties"/g)).toHaveLength(1);
  });
  it('says what is selected when there is nothing to edit, without inventing a page', () => {
    expect(html(null, 0)).toContain('Nothing selected');
    expect(html(null, 0)).toContain('Select a frame, a shape or text on the canvas to edit it.');
    expect(html(null, 3)).toContain('3 nodes selected');
    expect(html(null, 3)).toContain('Select one node to edit its properties.');
    expect(html(null, 0)).not.toMatch(/Page/);
    expect(html(null, 0)).toContain('data-testid="properties-empty"');
  });
  it('gives the image picker to the Fill section while the app state has it open for the node, through the view the page gave', () => {
    const closed: AssetsState = { items: [], loading: false, loaded: false, capped: false, request: 0 };
    const view = { mount: () => ({ update() {}, destroy() {} }) };
    expect(html(properties(), 1, { assets: { ...closed, target: 'n' }, imageView: view })).toContain('<div class="contents"></div>');
    expect(html(properties(), 1, { assets: closed, imageView: view })).not.toContain('<div class="contents"></div>');
  });
  it('draws the gradient and the image thumbnail the page gives, in the Fill section', () => {
    expect(html(properties({ kind: 'rect', sections: RECT, fillKind: 'linear', gradient }))).toContain('data-testid="prop-gradient-bar"');
    const image = properties({ kind: 'rect', sections: RECT, fillKind: 'image', image: { assetId: 'a'.repeat(64), fit: 'cover' } });
    expect(html(image, 1, { thumbnail: (id) => `/v1/assets/${id}` })).toContain('src="/v1/assets/' + 'a'.repeat(64) + '"');
    expect(html(image)).not.toContain('<img');
  });
});

// The two copies draw the same elements with the same classes, names, states and texts, in the same order (a class that is not here
// is a visible difference). What a class cannot say (the real pixels) is checked in a browser by pixels-e2e.mjs, with a tolerance of 0. The bind
// button is drawn by Bits UI in one copy and Base UI in the other: the ids and the tab stop that the two libraries write differently are left out.
describe('the React properties panel against the Svelte one', () => {
  const svelte = (p: Properties | null, selected: number) =>
    render(TestHost as never, { props: { of: PropertiesPanelSvelte, props: { properties: p, picker: null, selected, dispatch: () => {} } } as never }).body;
  const states: [string, Properties | null, number][] = [
    ['a frame', properties({ layout: 'row', padding: { top: 4, right: 4, bottom: 4, left: 8 }, border: { width: 2, color: LINE_B, position: 'outside' }, shadows: [shadow] }), 1],
    ['a text', properties({ kind: 'text', sections: TEXT, fontWeight: 700, textAlign: 'right' }), 1],
    ['a rect in an auto layout parent, locked', properties({ kind: 'rect', sections: { ...RECT, placement: true }, inLayout: true, positionLocked: true, notes: [{ note: 'marginIgnored' }] }), 1],
    ['a bound frame', properties({ bound: ['fill', 'opacity', 'radius'], bindings: [binding('fill'), binding('opacity'), binding('radius', { missing: true })] }), 1],
    ['bound to variables with no name', properties({ bound: ['fill', 'opacity'], bindings: [binding('fill', { name: '' }), binding('opacity', { name: '', missing: true })] }), 1],
    ['a gradient', properties({ kind: 'rect', sections: RECT, fillKind: 'linear', gradient }), 1],
    ['a frame, FRAME sections, with the Mixed padding', properties({ sections: FRAME, layout: 'column', padding: { top: 1, right: 2, bottom: 3, left: 4 } }), 1],
    ['nothing selected', null, 0],
    ['several selected', null, 4],
  ];
  for (const [name, p, selected] of states) {
    it(`has the same classes, names, states and texts: ${name}`, () => {
      expect(viewOf(html(p, selected))).toEqual(viewOf(svelte(p, selected)));
    });
  }
});
