// The image picker of the Fill section as the Inspector draws it: opened by the app state, a grid of tiles with the short id
// as the visible label, a Load more button for the cursor, and the notes for an empty, a loading and a failed list.
import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type { AssetInfo, AssetsState, Properties } from '@meldframe/protocol';
import TestHost from '@meldframe/web-chrome/TestHost.svelte';
import PropertiesPanel from './PropertiesPanel.svelte';

const base: Properties = { id: 'n', name: 'Hero', kind: 'rect', x: 0, y: 0, w: 10, h: 10, fill: '#112233', fillKind: 'solid', gradientAllowed: true, imageAllowed: true, minStops: 2, maxStops: 8, border: { width: 0, color: '#000000', position: 'inside' }, shadows: [], maxShadows: 4, radius: 0, layout: 'none', gap: 0, padding: { top: 0, right: 0, bottom: 0, left: 0 }, margin: { top: 0, right: 0, bottom: 0, left: 0 }, sw: 'fixed', sh: 'fixed', align: 'start', justify: 'start', absolute: false, inLayout: false, positionLocked: false, notes: [], sections: { radius: true, clip: false, fontSize: false, layout: false, placement: false }, text: '', fontSize: 16, opacity: 1, clip: false, bound: [], bindings: [] };
const id = (n: number) => n.toString(16).padStart(2, '0').repeat(32);
const asset = (n: number, over: Partial<AssetInfo> = {}): AssetInfo => ({ id: id(n), bytes: 2048 * n, mime: 'image/png', ...over });
const closed: AssetsState = { items: [], loading: false, loaded: false, capped: false, request: 0 };
const html = (assets: AssetsState, properties: Properties = base) =>
  render(TestHost as never, { props: { of: PropertiesPanel, props: { properties, picker: null, assets, selected: 1, dispatch: () => {} } } as never }).body;
const opened = (over: Partial<AssetsState> = {}): AssetsState => ({ ...closed, target: 'n', ...over });

describe('the image picker', () => {
  it('is not drawn while the app state has it closed, or open for another node', () => {
    expect(html(closed)).not.toContain('data-testid="image-picker"');
    expect(html(opened({ target: 'other' }))).not.toContain('data-testid="image-picker"');
  });
  it('is drawn in the Fill section while the app state has it open for the node, with a visible title and a close button', () => {
    const out = html(opened({ loading: true }));
    expect(out).toContain('data-testid="image-picker"');
    expect(out).toContain('role="group"');
    expect(out).toContain('Choose an image');
    expect(out).toContain('aria-label="Close image picker"');
  });
  it('says it is loading before the first page, and shows no empty note then', () => {
    const out = html(opened({ loading: true }));
    expect(out).toContain('data-testid="image-picker-loading"');
    expect(out).not.toContain('data-testid="image-picker-empty"');
  });
  it('says there are no images in an empty workspace, in the words of the glossary', () => {
    const out = html(opened({ loaded: true }));
    expect(out).toContain('No images in this workspace yet. An agent can upload one.');
    expect(out).not.toContain('data-testid="image-tile"');
    expect(out).not.toContain('Load more');
  });
  it('lists a tile for each asset with the short id as its visible label and its details as its name, and the thumbnail from the asset route', () => {
    const out = html(opened({ loaded: true, items: [asset(1), asset(171, { mime: 'image/jpeg', bytes: 5 * 1024 * 1024 })] }));
    expect(out.match(/data-testid="image-tile"/g)).toHaveLength(2);
    expect(out).toContain('aria-label="Image 01010101, PNG, 2 KB"');
    expect(out).toContain('aria-label="Image abababab, JPEG, 5.0 MB"');
    expect(out).toContain('>01010101<');
    expect(out).toContain(`src="/v1/assets/${id(1)}"`);
    expect(out).toContain('aria-label="Images in this workspace"');
    // The thumbnail is decoration: the tile has the name.
    expect(out).toContain('alt=""');
  });
  it('is one tab stop: the first tile only, until the cursor moves', () => {
    const out = html(opened({ loaded: true, items: [asset(1), asset(2), asset(3)] }));
    const stops = [...out.matchAll(/<button[^>]*data-testid="image-tile"[^>]*>/g)].map((m) => /tabindex="0"/.test(m[0]));
    expect(stops).toEqual([true, false, false]);
  });
  it('marks the tile of the image the node has already as current, and no tile is a toggle', () => {
    const node: Properties = { ...base, fillKind: 'image', image: { assetId: id(2), fit: 'cover' } };
    const out = html(opened({ loaded: true, items: [asset(1), asset(2)] }), node);
    const tiles = [...out.matchAll(/<button[^>]*data-testid="image-tile"[^>]*>/g)].map((m) => m[0]);
    expect(tiles.map((t) => /aria-current="true"/.test(t))).toEqual([false, true]);
    expect(tiles.some((t) => t.includes('aria-pressed'))).toBe(false);
  });
  it('has Load more only when there is a cursor, and says Loading while the next page comes', () => {
    expect(html(opened({ loaded: true, items: [asset(1)] }))).not.toContain('image-picker-more');
    const more = html(opened({ loaded: true, items: [asset(1)], next: id(1) }));
    expect(more).toContain('data-testid="image-picker-more"');
    expect(more).toContain('Load more');
    const busy = html(opened({ loaded: true, items: [asset(1)], next: id(1), loading: true }));
    expect(busy).toContain('Loading…');
  });
  it('says that the list stopped at its cap, and has no Load more then', () => {
    const out = html(opened({ loaded: true, capped: true, items: [asset(1), asset(2)] }));
    expect(out).toContain('data-testid="image-picker-capped"');
    expect(out).toContain('The first 2 images are listed');
    expect(out).not.toContain('image-picker-more');
    expect(html(opened({ loaded: true, items: [asset(1)] }))).not.toContain('image-picker-capped');
  });
  it('says what failed, as an alert, with a way to try again, and keeps the tiles read', () => {
    const out = html(opened({ loaded: true, items: [asset(1)], error: 'unavailable' }));
    expect(out).toContain('role="alert"');
    expect(out).toContain('Images could not be loaded (unavailable).');
    expect(out).toContain('data-testid="image-picker-retry"');
    expect(out).toContain('data-testid="image-tile"');
  });
});
