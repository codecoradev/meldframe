// The React copy of the Fill section, checked as fill.test.ts and image-picker.test.ts check the Svelte one: the markup of a server
// render, in Node. Clicks, the focus and the image picker that the section hosts are the browser scripts' (image-fill-e2e.mjs,
// with the panel, and editor-e2e.mjs, both with MELDFRAME_UI=react).
import type { AssetsState, Picker, Properties } from '@meldframe/protocol';
import TestHost from '@meldframe/web-chrome/TestHost.svelte';
import { TooltipProvider } from '@meldframe/web-chrome/react/Tooltip';
import { renderToStaticMarkup } from 'react-dom/server';
import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import FillSectionSvelte from '../FillSection.svelte';
import { FillSection } from './FillSection';
import { ForeignSlot } from './ForeignSlot';
import { binding, gradient, properties } from './fixture';

// The route of an asset is the host's (a view may not import web-assets): the test gives the one the page does, for ids of 64 hex digits.
const thumbnail = (id: string) => (/^[0-9a-f]{64}$/.test(id) ? `/v1/assets/${id}` : null);
const html = (p: Properties, picker: Picker | null = null, assets?: AssetsState, imageView?: Parameters<typeof FillSection>[0]['imageView'], thumb: typeof thumbnail | 'none' = thumbnail) =>
  renderToStaticMarkup(
    <TooltipProvider>
      <FillSection properties={p} picker={picker} assets={assets} dispatch={() => {}} imageView={imageView} thumbnail={thumb === 'none' ? undefined : thumb} />
    </TooltipProvider>,
  );
const rect = (over: Partial<Properties> = {}) => properties({ kind: 'rect', ...over });

// The labels of the options of a segmented control, and those that are checked.
const options = (out: string, group: string, only: 'checked' | 'all'): string[] => {
  const from = out.slice(out.indexOf(`aria-label="${group}"`));
  const re = only === 'checked' ? /aria-checked="true"[^>]*title="([^"]*)"/g : /role="radio"[^>]*title="([^"]*)"/g;
  return [...from.slice(0, from.indexOf('</div>')).matchAll(re)].map((m) => m[1]!);
};
const checked = (out: string, group: string) => options(out, group, 'checked');
const labels = (out: string, group: string) => options(out, group, 'all');
const off = (out: string, id: string) => (out.match(new RegExp(`<button[^>]*data-testid="${id}"[^>]*>`, 'g')) ?? []).map((b) => /\sdisabled(?=[\s=>])/.test(b));

describe('the React Fill section', () => {
  it('has the type switch with solid, linear, radial, image and none, the chosen one checked', () => {
    const out = html(rect());
    expect(out).toContain('aria-label="Fill type"');
    expect(labels(out, 'Fill type')).toEqual(['Solid', 'Linear', 'Radial', 'Image', 'None']);
    expect(checked(out, 'Fill type')).toEqual(['Solid']);
    expect(checked(html(rect({ fillKind: 'linear', gradient })), 'Fill type')).toEqual(['Linear']);
    expect(checked(html(rect({ fill: 'none', fillKind: 'none' })), 'Fill type')).toEqual(['None']);
  });
  it('keeps the solid field and test ids, and has a bind button for it', () => {
    const out = html(rect());
    expect(out).toContain('data-testid="prop-fill"');
    expect(out).toContain('data-testid="prop-fill-bind"');
    expect(out).toContain('aria-label="Bind Fill to a variable"');
  });
  it('shows the chip of the variable and a way to detach it when the fill is bound, and no field', () => {
    const out = html(rect({ bound: ['fill'], bindings: [binding('fill', { name: 'color/brand' })] }));
    expect(out).toContain('data-testid="prop-fill-bound"');
    expect(out).toContain('aria-label="Detach color/brand"');
    expect(out).not.toContain('data-testid="prop-fill-bind"');
    expect(html(rect({ bound: ['fill'], bindings: [binding('fill', { name: 'ghost', missing: true })] }))).toContain('ghost (removed)');
  });
  it('shows no colour field for none', () => {
    expect(html(rect({ fill: 'none', fillKind: 'none' }))).not.toContain('data-testid="prop-fill"');
  });
  it('shows the angle, the bar and a row for each stop of a linear gradient, and no angle for a radial one', () => {
    const out = html(rect({ fillKind: 'linear', gradient }));
    expect(out).toContain('data-testid="prop-gradient-angle"');
    expect(out).toContain('data-testid="prop-gradient-bar"');
    expect(out).toContain('aria-label="Gradient, 3 stops"');
    expect(out.match(/data-testid="prop-gradient-stop"/g)).toHaveLength(3);
    expect(out).toContain('data-testid="prop-gradient-add"');
    expect(html(rect({ fillKind: 'radial', gradient: { ...gradient, kind: 'radial' } }))).not.toContain('data-testid="prop-gradient-angle"');
  });
  it('offers no gradient and no image on text', () => {
    expect(labels(html(properties({ kind: 'text', gradientAllowed: false, imageAllowed: false })), 'Fill type')).toEqual(['Solid', 'None']);
  });
  it('takes the limits of the stops from the app layer: no add at the most, no remove at the fewest', () => {
    const at = (minStops: number, maxStops: number) => html(rect({ fillKind: 'linear', gradient, minStops, maxStops }));
    expect(off(at(2, 3), 'prop-gradient-add')).toEqual([true]);
    expect(off(at(2, 8), 'prop-gradient-add')).toEqual([false]);
    expect(off(at(3, 8), 'prop-gradient-remove')).toEqual([true, true, true]);
    expect(off(at(2, 8), 'prop-gradient-remove')).toEqual([false, false, false]);
  });
  it('shows the image fill with its short id, a way to change it and its fit, and no colour field', () => {
    const out = html(rect({ fillKind: 'image', image: { assetId: 'a'.repeat(64), fit: 'cover' } }));
    expect(checked(out, 'Fill type')).toEqual(['Image']);
    expect(out).toContain('data-testid="prop-fill-image"');
    expect(out).not.toContain('data-testid="prop-fill"');
    expect(out).not.toContain('data-testid="prop-fill-bind"');
    expect(out).toContain('Image aaaaaaaa');
    expect(out).toContain('data-testid="prop-fill-image-change"');
    expect(labels(out, 'Image fit')).toEqual(['Cover', 'Contain', 'Fill']);
    expect(checked(out, 'Image fit')).toEqual(['Cover']);
    expect(out).toContain('src="/v1/assets/' + 'a'.repeat(64) + '"');
    expect(out).not.toContain('data-testid="image-picker"');
  });
  it('draws the thumbnail the page gives, and none when the page gives none for the id (the file is the sender\'s, not trusted) or no function', () => {
    const hostile = rect({ fillKind: 'image', image: { assetId: '../../etc/passwd', fit: 'cover' } });
    const out = html(hostile);
    expect(out).toContain('data-testid="prop-fill-image"');
    expect(out).not.toContain('<img');
    expect(html(rect({ fillKind: 'image', image: { assetId: 'a'.repeat(64), fit: 'cover' } }), null, undefined, undefined, 'none')).not.toContain('<img');
  });
});

describe('the image picker the React Fill section hosts', () => {
  const closed: AssetsState = { items: [], loading: false, loaded: false, capped: false, request: 0 };
  const view = { mount: () => ({ update() {}, destroy() {} }) };
  it('is a slot only while the app state has it open for this node, and none when the page gave no view', () => {
    const slot = '<div class="contents"></div>';
    expect(html(rect(), null, { ...closed, target: 'n' }, view)).toContain(slot);
    expect(html(rect(), null, closed, view)).not.toContain(slot);
    expect(html(rect(), null, { ...closed, target: 'other' }, view)).not.toContain(slot);
    expect(html(rect(), null, { ...closed, target: 'n' })).not.toContain(slot);
  });
  it('is drawn between the type switch and the fields of the type, as the Svelte section draws it', () => {
    const out = html(rect(), null, { ...closed, target: 'n' }, view);
    expect(out.indexOf('aria-label="Fill type"')).toBeLessThan(out.indexOf('<div class="contents"></div>'));
    expect(out.indexOf('<div class="contents"></div>')).toBeLessThan(out.indexOf('data-testid="prop-fill"'));
  });
  it('the slot is a host that draws nothing of its own', () => {
    expect(renderToStaticMarkup(<ForeignSlot view={view} props={{}} />)).toBe('<div class="contents"></div>');
  });
});

// The two copies draw the same elements with the same classes in the same order (a class that is not here is a visible
// difference). What a class cannot say (the real pixels) is checked in a browser by pixels-e2e.mjs, with a tolerance of 0.
describe('the React Fill section against the Svelte one', () => {
  // The Svelte section takes the chip as a snippet, which a test cannot pass; the bound states are compared through the whole panel (properties.test.tsx).
  const svelte = (p: Properties) => render(TestHost as never, { props: { of: FillSectionSvelte, props: { properties: p, picker: null, dispatch: () => {}, chip: () => {} } } as never }).body.replace(/<!--.*?-->/g, '');
  const classes = (out: string) => [...out.matchAll(/class="([^"]*)"/g)].map((m) => m[1]!.split(/\s+/).filter(Boolean).sort().join(' '));
  const attrs = (out: string, name: string) => [...out.matchAll(new RegExp(`\\s${name}="([^"]*)"`, 'g'))].map((m) => m[1]!).filter((v) => !/^(bits|base-ui)-/.test(v));
  const names = ['id', 'role', 'data-testid', 'aria-label', 'aria-checked', 'aria-haspopup', 'title', 'src', 'value'];
  const style = (out: string) => attrs(out, 'style').map((s) => s.replace(/[\s;]/g, ''));
  const states: [string, Properties][] = [
    ['solid', rect()],
    ['linear', rect({ fillKind: 'linear', gradient })],
    ['radial', rect({ fillKind: 'radial', gradient: { ...gradient, kind: 'radial' } })],
    ['image', rect({ fillKind: 'image', image: { assetId: 'a'.repeat(64), fit: 'contain' } })],
    ['none', rect({ fill: 'none', fillKind: 'none' })],
  ];
  for (const [name, p] of states) {
    it(`has the same classes, roles, names, test ids and styles: ${name}`, () => {
      const a = svelte(p);
      const b = html(p);
      expect(classes(b)).toEqual(classes(a));
      for (const attr of names) expect(attrs(b, attr), attr).toEqual(attrs(a, attr));
      expect(style(b)).toEqual(style(a));
    });
  }
});
