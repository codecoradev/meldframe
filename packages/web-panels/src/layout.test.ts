import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type { Properties } from '@meldframe/protocol';
import TestHost from '@meldframe/web-chrome/TestHost.svelte';
import PropertiesPanel from './PropertiesPanel.svelte';

const zero = { top: 0, right: 0, bottom: 0, left: 0 };
const base: Properties = { id: 'n', name: 'Hero', kind: 'frame', x: 0, y: 0, w: 10, h: 10, fill: '#112233', fillKind: 'solid', gradientAllowed: true, minStops: 2, maxStops: 8, border: { width: 0, color: '#000000', position: 'inside' }, shadows: [], maxShadows: 4, radius: 0, layout: 'none', gap: 8, padding: zero, margin: zero, sw: 'fixed', sh: 'fixed', align: 'start', justify: 'start', absolute: false, inLayout: false, notes: [], text: '', fontSize: 16, opacity: 1, clip: true, bound: [], bindings: [] };
const html = (properties: Properties) => render(TestHost as never, { props: { of: PropertiesPanel, props: { properties, picker: null, selected: 1, dispatch: () => {} } } as never }).body;

describe('the layout fields as the boards draw them', () => {
  it('shows the direction of a frame and, for a free frame, nothing more of the layout', () => {
    const out = html(base);
    expect(out).toContain('aria-label="Layout direction"');
    for (const w of ['Free', 'Row', 'Column']) expect(out).toContain(w);
    for (const id of ['prop-gap', 'prop-padding', 'prop-align', 'prop-justify']) expect(out).not.toContain(`data-testid="${id}"`);
  });
  it('shows the gap, the padding and the alignment of a row or a column', () => {
    const out = html({ ...base, layout: 'row' });
    for (const id of ['prop-gap', 'prop-padding', 'prop-padding-top', 'prop-padding-right', 'prop-padding-bottom', 'prop-padding-left', 'prop-align', 'prop-justify']) expect(out).toContain(`data-testid="${id}"`);
    expect(out).toContain('Between');
  });
  it('shows the padding the sides share, and a Mixed placeholder in an empty field when they differ', () => {
    const field = (out: string) => out.match(/<input[^>]*data-testid="prop-padding"[^>]*>/)![0];
    const same = field(html({ ...base, layout: 'row', padding: { top: 4, right: 4, bottom: 4, left: 4 }, paddingUniform: 4 }));
    const mixed = field(html({ ...base, layout: 'row', padding: { top: 4, right: 4, bottom: 4, left: 8 } }));
    expect(same).toContain('value="4"');
    expect(same).not.toContain('placeholder');
    expect(mixed).toContain('value=""');
    expect(mixed).toContain('placeholder="Mixed"');
  });
  it('has no layout section for a node that is not a frame', () => {
    expect(html({ ...base, kind: 'rect' })).not.toContain('aria-label="Layout direction"');
  });
  it('sizes the width and the height in the Size section, the chosen mode checked', () => {
    const out = html({ ...base, sw: 'hug' });
    expect(out).toContain('aria-label="Width sizing"');
    expect(out).toContain('aria-label="Height sizing"');
    for (const w of ['Fixed', 'Hug', 'Fill']) expect(out).toContain(w);
    expect(out.indexOf('aria-label="Size"')).toBeLessThan(out.indexOf('aria-label="Width sizing"'));
  });
  it('shows the margin and absolute only inside an auto layout parent', () => {
    expect(html(base)).not.toContain('data-testid="prop-margin-top"');
    const out = html({ ...base, inLayout: true });
    for (const s of ['top', 'right', 'bottom', 'left']) expect(out).toContain(`data-testid="prop-margin-${s}"`);
    expect(out).toContain('data-testid="prop-absolute"');
  });
  it('keeps the margin and absolute fields while the node has them, also when the parent stopped laying out', () => {
    // A node with a margin of 8 and absolute inside a row frame, then the parent is set to Free: the app layer says the
    // node is no longer in a layout and sends the two notes. The fields that clear them must still be there.
    const margin = { top: 8, right: 0, bottom: 0, left: 0 };
    const freed: Properties = { ...base, kind: 'rect', margin, absolute: true, inLayout: false, notes: [{ note: 'marginIgnored' }, { note: 'absoluteIgnored' }] };
    const out = html(freed);
    expect(out).toContain('data-testid="prop-margin-top"');
    expect(out).toContain('data-testid="prop-absolute"');
    expect(out).toMatch(/data-testid="prop-margin-top"[^>]*value="8"|value="8"[^>]*data-testid="prop-margin-top"/);
    expect(out.match(/role="note"/g)).toHaveLength(2);
    // Cleared, with nothing left to say: the section goes again.
    const clear = html({ ...freed, margin: { top: 0, right: 0, bottom: 0, left: 0 }, absolute: false, notes: [] });
    expect(clear).not.toContain('data-testid="prop-margin-top"');
  });
  it('words each note the app layer sends, as a note', () => {
    const out = html({ ...base, notes: [{ note: 'fillUnderHug', axis: 'width' }, { note: 'marginIgnored' }] });
    expect(out.match(/role="note"/g)).toHaveLength(2);
    expect(out).toContain('Fill width needs a parent with auto layout');
    expect(out).toContain('The margin takes no room here');
    expect(html(base)).not.toContain('role="note"');
  });
});
