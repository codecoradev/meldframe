import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type { Properties } from '@meldframe/protocol';
import TestHost from '@meldframe/web-chrome/TestHost.svelte';
import PropertiesPanel from './PropertiesPanel.svelte';

const shadow = { x: 0, y: 4, blur: 8, spread: 0, color: '#00000040' };
const base: Properties = { id: 'n', name: 'Hero', kind: 'rect', x: 0, y: 0, w: 10, h: 10, fill: '#112233', fillKind: 'solid', gradientAllowed: true, imageAllowed: true, minStops: 2, maxStops: 8, border: { width: 0, color: '#000000', position: 'inside' }, shadows: [], maxShadows: 4, radius: 0, layout: 'none', gap: 0, padding: { top: 0, right: 0, bottom: 0, left: 0 }, margin: { top: 0, right: 0, bottom: 0, left: 0 }, sw: 'fixed', sh: 'fixed', align: 'start', justify: 'start', absolute: false, inLayout: false, positionLocked: false, notes: [], text: '', fontSize: 16, opacity: 1, clip: false, bound: [], bindings: [] };
// The labels of the options of a segmented control that are checked.
const checked = (out: string, group: string): string[] => {
  const from = out.slice(out.indexOf(`aria-label="${group}"`));
  return [...from.slice(0, from.indexOf('</div>')).matchAll(/aria-checked="true"[^>]*title="([^"]*)"/g)].map((m) => m[1]!);
};
const addButton = (out: string) => out.match(/<button[^>]*data-testid="prop-shadow-add"[^>]*>/)![0];
// The attribute, not the `disabled:` variants of the classes.
const addDisabled = (out: string) => /\sdisabled(?=[\s=>])/.test(addButton(out));
const html = (properties: Properties) => render(TestHost as never, { props: { of: PropertiesPanel, props: { properties, picker: null, selected: 1, dispatch: () => {} } } as never }).body;

describe('the Border and Shadow sections as the boards draw them', () => {
  it('comes after Fill, in the order Fill, Border, Shadow', () => {
    const out = html(base);
    const at = ['Fill', 'Border', 'Shadow'].map((t) => out.indexOf(`aria-label="${t}"`));
    expect(at.every((i) => i >= 0)).toBe(true);
    expect([...at].sort((a, b) => a - b)).toEqual(at);
  });
  it('shows the width and colour of the border and a position only when there is a border', () => {
    expect(html(base)).toContain('data-testid="prop-border-width"');
    expect(html(base)).toContain('data-testid="prop-border-color"');
    expect(html(base)).not.toContain('aria-label="Border position"');
    const out = html({ ...base, border: { width: 2, color: '#ff0000', position: 'outside' } });
    expect(out).toContain('aria-label="Border position"');
    expect(checked(out, 'Border position')).toEqual(['Outside']);
  });
  it('has a row for each shadow with its four numbers, a colour and a remove button, and an add button', () => {
    const out = html({ ...base, shadows: [shadow, { ...shadow, blur: 20 }] });
    expect(out.match(/data-testid="prop-shadow"/g)).toHaveLength(2);
    for (const f of ['x', 'y', 'blur', 'spread']) expect(out.match(new RegExp(`data-testid="prop-shadow-${f}"`, 'g'))).toHaveLength(2);
    expect(out).toContain('aria-label="Remove shadow 2"');
    expect(out).toContain('data-testid="prop-shadow-add"');
  });
  it('stops offering to add at the limit the app layer gives, and not before', () => {
    expect(addDisabled(html({ ...base, shadows: [shadow, shadow], maxShadows: 2 }))).toBe(true);
    expect(addDisabled(html({ ...base, shadows: [shadow, shadow], maxShadows: 4 }))).toBe(false);
  });
});
