// The React copy of the image picker, checked as image-picker.test.ts checks the Svelte one (the same states, the same words) and
// against it: the markup of a server render, in Node. Clicks, the keyboard, the focus and the thumbnails are the browser scripts'
// (image-fill-e2e.mjs with MELDFRAME_UI=react, and pixels-e2e.mjs, which images the open picker on both copies).
import type { AssetInfo, AssetsState, Properties } from '@meldframe/protocol';
import TestHost from '@meldframe/web-chrome/TestHost.svelte';
import { TooltipProvider } from '@meldframe/web-chrome/react/Tooltip';
import { renderToStaticMarkup } from 'react-dom/server';
import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import ImageFillPickerSvelte from '../ImageFillPicker.svelte';
import { ImageFillPicker } from './ImageFillPicker';
import { viewOf } from './compare';
import { properties } from './fixture';

const id = (n: number) => n.toString(16).padStart(2, '0').repeat(32);
const asset = (n: number, over: Partial<AssetInfo> = {}): AssetInfo => ({ id: id(n), bytes: 2048 * n, mime: 'image/png', ...over });
const closed: AssetsState = { items: [], loading: false, loaded: false, capped: false, request: 0 };
const opened = (over: Partial<AssetsState> = {}): AssetsState => ({ ...closed, target: 'n', ...over });
const node = properties({ kind: 'rect' });
// The route of an asset is the host's (a view may not import web-assets): the test gives the one the page does.
const route = (assetId: string) => `/v1/assets/${assetId}`;
const html = (assets: AssetsState, p: Properties = node, thumbnail: (assetId: string) => string | null = route) =>
  renderToStaticMarkup(
    <TooltipProvider>
      <ImageFillPicker properties={p} assets={assets} dispatch={() => {}} thumbnail={thumbnail} />
    </TooltipProvider>,
  );
const svelte = (assets: AssetsState, p: Properties = node) =>
  render(TestHost as never, { props: { of: ImageFillPickerSvelte, props: { properties: p, assets, dispatch: () => {} } } as never }).body;
const tiles = (out: string) => [...out.matchAll(/<button[^>]*data-testid="image-tile"[^>]*>/g)].map((m) => m[0]);

describe('the React image picker', () => {
  it('is a group with a visible title and a close button', () => {
    const out = html(opened({ loading: true }));
    expect(out).toContain('data-testid="image-picker"');
    expect(out).toContain('role="group"');
    expect(out).toContain('aria-labelledby="image-picker-title"');
    expect(out).toContain('Choose an image');
    expect(out).toContain('aria-label="Close image picker"');
    expect(out).toContain('data-testid="image-picker-close"');
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
  it('lists a tile for each asset with the short id as its visible label and its details as its name, and the thumbnail the page gives', () => {
    const out = html(opened({ loaded: true, items: [asset(1), asset(171, { mime: 'image/jpeg', bytes: 5 * 1024 * 1024 })] }));
    expect(tiles(out)).toHaveLength(2);
    expect(out).toContain('aria-label="Image 01010101, PNG, 2 KB"');
    expect(out).toContain('aria-label="Image abababab, JPEG, 5.0 MB"');
    expect(out).toContain('>01010101<');
    expect(out).toContain(`src="/v1/assets/${id(1)}"`);
    expect(out).toContain('aria-label="Images in this workspace"');
    expect(out).toContain('alt=""');
  });
  it('loads thumbnails lazily and decodes them off the main thread, as the Svelte picker does (the markup comparison below does not see these two attributes)', () => {
    const items = [asset(1), asset(2), asset(3)];
    for (const [name, out] of [['React', html(opened({ loaded: true, items }))], ['Svelte', svelte(opened({ loaded: true, items }))]] as const) {
      const imgs = [...out.matchAll(/<img[^>]*>/g)].map((m) => m[0]);
      expect(imgs, name).toHaveLength(3);
      for (const img of imgs) {
        expect(img, name).toContain('loading="lazy"');
        expect(img, name).toContain('decoding="async"');
      }
    }
  });
  it('draws no image for an id the page gives no route for (the id is the sender\'s, not trusted), and says there is no preview', () => {
    const out = html(opened({ loaded: true, items: [asset(1)] }), node, () => null);
    expect(out).not.toContain('<img');
    expect(out).toContain('No preview');
    // Not a failed load: nothing is counted and there is no Retry previews.
    expect(out).not.toContain('data-testid="image-picker-broken"');
  });
  it('is one tab stop: the first tile only, until the cursor moves', () => {
    const out = html(opened({ loaded: true, items: [asset(1), asset(2), asset(3)] }));
    expect(tiles(out).map((t) => /tabindex="0"/.test(t))).toEqual([true, false, false]);
  });
  it('marks the tile of the image the node has already as current, and no tile is a toggle', () => {
    const has = properties({ kind: 'rect', fillKind: 'image', image: { assetId: id(2), fit: 'cover' } });
    const t = tiles(html(opened({ loaded: true, items: [asset(1), asset(2)] }), has));
    expect(t.map((b) => /aria-current="true"/.test(b))).toEqual([false, true]);
    expect(t.some((b) => b.includes('aria-pressed'))).toBe(false);
  });
  it('has Load more only when there is a cursor, and says Loading while the next page comes', () => {
    expect(html(opened({ loaded: true, items: [asset(1)] }))).not.toContain('image-picker-more');
    const more = html(opened({ loaded: true, items: [asset(1)], next: id(1) }));
    expect(more).toContain('data-testid="image-picker-more"');
    expect(more).toContain('Load more');
    expect(html(opened({ loaded: true, items: [asset(1)], next: id(1), loading: true }))).toContain('Loading…');
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
  it('draws text as the app layer gave it, with no cleaning of the error (rule U9 is the app layer\'s)', () => {
    expect(html(opened({ loaded: true, error: 'a  b' }))).toContain('Images could not be loaded (a  b).');
  });
  it('writes each sentence as one text node, so that a clipped or wrapped line is laid out as the Svelte copy lays it out', () => {
    const out = html(opened({ loaded: true, capped: true, items: [asset(1), asset(2)], error: 'unavailable' }));
    // React separates two adjacent text nodes with a comment in a server render; none of these sentences is made of two.
    expect(out).not.toContain('<!-- -->');
  });
});

// The two copies draw the same elements with the same classes, names, states and texts, in the same order (a class that is not here
// is a visible difference). What a class cannot say (the real pixels) is checked in a browser by pixels-e2e.mjs, with a tolerance of 0.
describe('the React image picker against the Svelte one', () => {
  const has = properties({ kind: 'rect', fillKind: 'image', image: { assetId: id(2), fit: 'contain' } });
  const many = Array.from({ length: 7 }, (_, i) => asset(i + 1));
  const states: [string, AssetsState, Properties][] = [
    ['loading', opened({ loading: true }), node],
    ['empty', opened({ loaded: true }), node],
    ['two tiles', opened({ loaded: true, items: [asset(1), asset(171, { mime: 'image/jpeg', bytes: 5 * 1024 * 1024 })] }), node],
    ['seven tiles, one current', opened({ loaded: true, items: many }), has],
    ['Load more', opened({ loaded: true, items: many, next: id(7) }), node],
    ['Load more while the next page comes', opened({ loaded: true, items: many, next: id(7), loading: true }), node],
    ['the cap', opened({ loaded: true, capped: true, items: many }), node],
    ['a failed list with the tiles read', opened({ loaded: true, items: [asset(1)], error: 'unavailable' }), node],
    ['a failed first page', opened({ error: 'unavailable' }), node],
  ];
  for (const [name, assets, p] of states) {
    it(`has the same classes, names, states and texts: ${name}`, () => {
      expect(viewOf(html(assets, p))).toEqual(viewOf(svelte(assets, p)));
    });
  }
});
