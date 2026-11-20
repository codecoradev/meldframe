import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type { Properties } from '@meldframe/protocol';
import TestHost from '@meldframe/web-chrome/TestHost.svelte';
import PropertiesPanel from './PropertiesPanel.svelte';

const base: Properties = { id: 'n', name: 'Hero', kind: 'frame', x: 10, y: 20, w: 300, h: 120, fill: '#112233', fillKind: 'solid', gradientAllowed: true, minStops: 2, maxStops: 8, border: { width: 0, color: '#000000', position: 'inside' }, shadows: [], maxShadows: 4, radius: 8, layout: 'none', gap: 0, padding: { top: 0, right: 0, bottom: 0, left: 0 }, margin: { top: 0, right: 0, bottom: 0, left: 0 }, sw: 'fixed', sh: 'fixed', align: 'start', justify: 'start', absolute: false, inLayout: false, notes: [], text: '', fontSize: 16, opacity: 0.5, clip: true, bound: [], bindings: [] };
const b = (field: 'fill' | 'opacity' | 'radius', over = {}) => ({ field, variable: 'v1', name: 'space/md', missing: false, ...over });
const html = (properties: Properties | null, selected = properties ? 1 : 0) =>
  render(TestHost as never, { props: { of: PropertiesPanel, props: { properties, selected, dispatch: () => {} } } as never }).body;

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
  it('has a bind button on each field that takes a variable, and none on a field that is bound already', () => {
    const out = html(base);
    for (const f of ['fill', 'opacity', 'radius']) expect(out).toContain(`data-testid="prop-${f}-bind"`);
    for (const f of ['fill', 'opacity', 'radius'] as const) {
      const bound = html({ ...base, bound: [f], bindings: [b(f)] });
      expect(bound, f).not.toContain(`data-testid="prop-${f}-bind"`);
      for (const other of ['fill', 'opacity', 'radius'].filter((x) => x !== f)) expect(bound, `${f} bound, ${other}`).toContain(`data-testid="prop-${other}-bind"`);
    }
    expect(html({ ...base, kind: 'text' })).not.toContain('data-testid="prop-radius-bind"');
    expect(html({ ...base, kind: 'text' })).toContain('data-testid="prop-opacity-bind"');
  });
  it('is one group named Properties with a rule above it, and its sections are the landmarks inside', () => {
    const out = html(base);
    expect(out).toMatch(/<div[^>]*role="group"[^>]*aria-label="Properties"/);
    expect(out).toContain('border-t border-hairline');
    expect(out.match(/<section/g)?.length).toBeGreaterThanOrEqual(4);
    expect(out.match(/aria-label="Properties"/g)).toHaveLength(1);
  });
  it('says what is selected when there is nothing to edit, without inventing a page', () => {
    expect(html(null, 0)).toContain('Nothing selected');
    expect(html(null, 3)).toContain('3 nodes selected');
    expect(html(null, 0)).not.toMatch(/Page/);
  });
});
