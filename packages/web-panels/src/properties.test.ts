import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type { Properties } from '@meldframe/protocol';
import PropertiesPanel from './PropertiesPanel.svelte';

const base: Properties = { id: 'n', name: 'Hero', kind: 'frame', x: 10, y: 20, w: 300, h: 120, fill: '#112233', radius: 8, text: '', fontSize: 16, opacity: 0.5, clip: true, bound: [], bindings: [] };
const b = (field: 'fill' | 'opacity' | 'radius', over = {}) => ({ field, variable: 'v1', name: 'space/md', missing: false, ...over });
const html = (properties: Properties | null, selected = properties ? 1 : 0) =>
  render(PropertiesPanel as never, { props: { properties, selected, dispatch: () => {} } as never }).body;

describe('the properties panel as the Properties and Inspector boards draw it', () => {
  it('has the sections Position, Size, Appearance and Fill in that order', () => {
    const out = html(base);
    const at = ['Position', 'Size', 'Appearance', 'Fill'].map((t) => out.indexOf(`aria-label="${t}"`));
    expect(at.every((i) => i >= 0)).toBe(true);
    expect([...at].sort((a, b) => a - b)).toEqual(at);
  });
  it('keeps the test ids of the fields and shows opacity as a percentage', () => {
    const out = html(base);
    for (const id of ['prop-name', 'prop-x', 'prop-y', 'prop-w', 'prop-h', 'prop-fill', 'prop-radius', 'prop-opacity', 'prop-clip']) expect(out).toContain(`data-testid="${id}"`);
    expect(out).toMatch(/data-testid="prop-opacity"[^>]*value="50"|value="50"[^>]*data-testid="prop-opacity"/);
  });
  it('offers the fields of the kind: no corners on text, a font size instead, and only a frame clips', () => {
    const text = html({ ...base, kind: 'text' });
    expect(text).toContain('data-testid="prop-fontSize"');
    expect(text).not.toContain('data-testid="prop-radius"');
    expect(text).not.toContain('data-testid="prop-clip"');
    expect(html({ ...base, kind: 'rect' })).not.toContain('data-testid="prop-clip"');
  });
  it('shows a bound fill as a chip with the variable name and a way to detach it', () => {
    const out = html({ ...base, bound: ['fill'], bindings: [b('fill', { name: 'color/brand' })] });
    expect(out).toContain('data-testid="prop-fill-bound"');
    expect(out).toContain('color/brand');
    expect(out).toContain('bg-chip-bg');
    expect(out).toContain('aria-label="Detach color/brand"');
  });
  it('shows a bound opacity and a bound corner radius as a full-width chip with a detach button each', () => {
    const out = html({ ...base, bound: ['opacity', 'radius'], bindings: [b('opacity'), b('radius')] });
    for (const f of ['opacity', 'radius']) {
      expect(out).toContain(`data-testid="prop-${f}-bound"`);
      expect(out).toContain(`data-testid="prop-${f}-detach"`);
    }
    expect(out).toContain('col-span-2');
    expect(out).toContain('aria-label="Detach space/md"');
  });
  it('shows a binding whose variable is gone as a red named chip that says so', () => {
    const out = html({ ...base, bound: ['fill'], bindings: [b('fill', { name: 'ghost', missing: true })] });
    expect(out).toContain('bg-danger-bg');
    expect(out).toContain('ghost (removed)');
  });
  it('says what is selected when there is nothing to edit, without inventing a page', () => {
    expect(html(null, 0)).toContain('Nothing selected');
    expect(html(null, 3)).toContain('3 nodes selected');
    expect(html(null, 0)).not.toMatch(/Page/);
  });
});
