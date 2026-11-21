import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type { Picker, Properties } from '@meldframe/protocol';
import TestHost from '@meldframe/web-chrome/TestHost.svelte';
import PropertiesPanel from './PropertiesPanel.svelte';

const base: Properties = { id: 'n', name: 'Hero', kind: 'rect', x: 0, y: 0, w: 10, h: 10, fill: '#112233', fillKind: 'solid', gradientAllowed: true, minStops: 2, maxStops: 8, border: { width: 0, color: '#000000', position: 'inside' }, shadows: [], maxShadows: 4, radius: 0, layout: 'none', gap: 0, padding: { top: 0, right: 0, bottom: 0, left: 0 }, margin: { top: 0, right: 0, bottom: 0, left: 0 }, sw: 'fixed', sh: 'fixed', align: 'start', justify: 'start', absolute: false, inLayout: false, positionLocked: false, notes: [], text: '', fontSize: 16, opacity: 1, clip: false, bound: [], bindings: [] };
const gradient = { kind: 'linear' as const, angle: 90, stops: [{ pos: 0, color: '#112233' }, { pos: 0.5, color: '#445566' }, { pos: 1, color: '#ffffff' }] };
// The section is drawn inside the properties panel, which hands it the chip of a bound field.
const html = (properties: Properties, picker: Picker | null = null) =>
  render(TestHost as never, { props: { of: PropertiesPanel, props: { properties, picker, selected: 1, dispatch: () => {} } } as never }).body;

// The labels of the options of a segmented control that are checked.
const checked = (out: string, group: string): string[] => {
  const from = out.slice(out.indexOf(`aria-label="${group}"`));
  return [...from.slice(0, from.indexOf('</div>')).matchAll(/aria-checked="true"[^>]*title="([^"]*)"/g)].map((m) => m[1]!);
};
const labels = (out: string, group: string): string[] => {
  const from = out.slice(out.indexOf(`aria-label="${group}"`));
  return [...from.slice(0, from.indexOf('</div>')).matchAll(/role="radio"[^>]*title="([^"]*)"/g)].map((m) => m[1]!);
};

describe('the Fill section as the boards draw it', () => {
  it('has the type switch with solid, linear, radial and none, the chosen one checked', () => {
    const out = html(base);
    expect(out).toContain('aria-label="Fill type"');
    expect(labels(out, 'Fill type')).toEqual(['Solid', 'Linear', 'Radial', 'None']);
    expect(checked(out, 'Fill type')).toEqual(['Solid']);
    expect(checked(html({ ...base, fillKind: 'linear', gradient }), 'Fill type')).toEqual(['Linear']);
    expect(checked(html({ ...base, fill: 'none', fillKind: 'none' }), 'Fill type')).toEqual(['None']);
  });
  it('keeps the solid field and test ids, and has a bind button for it', () => {
    const out = html(base);
    expect(out).toContain('data-testid="prop-fill"');
    expect(out).toContain('data-testid="prop-fill-bind"');
    expect(out).toContain('aria-label="Bind Fill to a variable"');
  });
  it('shows the chip of the variable and a way to detach it when the fill is bound', () => {
    const out = html({ ...base, bound: ['fill'], bindings: [{ field: 'fill', variable: 'v', name: 'color/brand', missing: false }] });
    expect(out).toContain('data-testid="prop-fill-bound"');
    expect(out).toContain('aria-label="Detach color/brand"');
  });
  it('shows no colour field for none', () => {
    expect(html({ ...base, fill: 'none', fillKind: 'none' })).not.toContain('data-testid="prop-fill"');
  });
  it('shows the angle, the bar and a row for each stop of a linear gradient, and no angle for a radial one', () => {
    const out = html({ ...base, fillKind: 'linear', gradientAllowed: true, minStops: 2, maxStops: 8, gradient });
    expect(out).toContain('data-testid="prop-gradient-angle"');
    expect(out).toContain('data-testid="prop-gradient-bar"');
    expect(out.match(/data-testid="prop-gradient-stop"/g)).toHaveLength(3);
    expect(out).toContain('data-testid="prop-gradient-add"');
    const radial = html({ ...base, fillKind: 'radial', gradientAllowed: true, minStops: 2, maxStops: 8, gradient: { ...gradient, kind: 'radial' } });
    expect(radial).not.toContain('data-testid="prop-gradient-angle"');
  });
  it('offers no gradient on text', () => {
    const out = html({ ...base, kind: 'text', gradientAllowed: false });
    expect(labels(out, 'Fill type')).toEqual(['Solid', 'None']);
  });
  it('takes the limits of the stops from the app layer: no add at the most, no remove at the fewest', () => {
    const at = (minStops: number, maxStops: number) => html({ ...base, fillKind: 'linear', gradient, minStops, maxStops });
    const off = (out: string, id: string) => (out.match(new RegExp(`<button[^>]*data-testid="${id}"[^>]*>`, 'g')) ?? []).map((b) => /\sdisabled(?=[\s=>])/.test(b));
    // Three stops: the most is 3, so Add stop is off; with 8 it is on.
    expect(off(at(2, 3), 'prop-gradient-add')).toEqual([true]);
    expect(off(at(2, 8), 'prop-gradient-add')).toEqual([false]);
    expect(off(at(2, 4), 'prop-gradient-add')).toEqual([false]);
    // Three stops: the fewest is 3, so every Remove is off; with 2 they are on.
    expect(off(at(3, 8), 'prop-gradient-remove')).toEqual([true, true, true]);
    expect(off(at(2, 8), 'prop-gradient-remove')).toEqual([false, false, false]);
    expect(off(at(4, 8), 'prop-gradient-remove')).toEqual([true, true, true]);
  });
  it('marks the image type when the node has an image fill, and offers it for no other', () => {
    const out = html({ ...base, fillKind: 'image', image: { assetId: 'a'.repeat(64), fit: 'cover' } });
    expect(checked(out, 'Fill type')).toEqual(['Image']);
    expect(labels(html(base), 'Fill type')).not.toContain('Image');
  });
  it('says an image is shown but cannot be picked yet', () => {
    const out = html({ ...base, fillKind: 'image', gradientAllowed: true, minStops: 2, maxStops: 8, image: { assetId: 'a'.repeat(64), fit: 'cover' } });
    expect(out).toContain('data-testid="prop-fill-image"');
    // The colour under an image does nothing here, so there is no field for it and no bind button.
    expect(out).not.toContain('data-testid="prop-fill"');
    expect(out).not.toContain('data-testid="prop-fill-bind"');
    expect(out).toContain('not available here yet');
  });
});
