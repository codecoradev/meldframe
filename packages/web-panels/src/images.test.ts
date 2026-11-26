import type { AssetInfo, Properties } from '@meldframe/protocol';
import { describe, expect, it } from 'vitest';
import { FOLLOW_START, askedForMore, closeAssetsAction, fitAction, fillTypes, followPages, gridMove, loadMoreAction, openAssetsAction, pickImageAction, retryAssetsAction, shortId, sizeText, tileLabel, typeText } from './images';

const ID = '3f2a91bc' + 'd'.repeat(56);
const asset: AssetInfo = { id: ID, bytes: 12_345, mime: 'image/png' };
const base = { id: 'n', kind: 'rect', gradientAllowed: true, imageAllowed: true, fillKind: 'solid' } as Properties;
const withImage = { ...base, fillKind: 'image', image: { assetId: 'a'.repeat(64), fit: 'contain' } } as Properties;

describe('what a tile says', () => {
  it('the short id is the first 8 characters, and there is no name', () => {
    expect(shortId(ID)).toBe('3f2a91bc');
    expect(tileLabel(asset)).toBe('Image 3f2a91bc, PNG, 12 KB');
  });
  it('names the four types for a person and passes another through', () => {
    expect(['image/png', 'image/jpeg', 'image/webp', 'image/gif'].map(typeText)).toEqual(['PNG', 'JPEG', 'WebP', 'GIF']);
    expect(typeText('image/avif')).toBe('image/avif');
  });
  it('writes a size in B, KB and MB', () => {
    expect([0, 1, 1023, 1024, 1536, 12_345, 1_048_575, 1_048_576, 5_452_595, 10 * 1024 * 1024].map(sizeText)).toEqual(['0 B', '1 B', '1023 B', '1 KB', '2 KB', '12 KB', '1024 KB', '1.0 MB', '5.2 MB', '10.0 MB']);
    expect(sizeText(-1)).toBe('');
    expect(sizeText(NaN)).toBe('');
  });
});

describe('the type switch', () => {
  it('offers the image before none wherever the app layer says the node takes one', () => {
    expect(fillTypes(base).map((t) => t.value)).toEqual(['solid', 'linear', 'radial', 'image', 'none']);
    expect(fillTypes({ ...base, kind: 'frame' }).map((t) => t.label)).toEqual(['Solid', 'Linear', 'Radial', 'Image', 'None']);
  });
  it('offers none of it but solid and none on text', () => {
    expect(fillTypes({ ...base, kind: 'text', gradientAllowed: false, imageAllowed: false }).map((t) => t.value)).toEqual(['solid', 'none']);
  });
  it('decides by the flag and not by the kind of the node', () => {
    expect(fillTypes({ ...base, imageAllowed: false }).map((t) => t.value)).toEqual(['solid', 'linear', 'radial', 'none']);
    expect(fillTypes({ ...base, kind: 'text', imageAllowed: true }).map((t) => t.value)).toContain('image');
  });
});

describe('the actions', () => {
  it('open, close, load more and retry carry nothing but the node', () => {
    expect(openAssetsAction(base)).toEqual({ type: 'openAssets', id: 'n' });
    expect(closeAssetsAction()).toEqual({ type: 'closeAssets' });
    expect(loadMoreAction()).toEqual({ type: 'loadMoreAssets' });
    expect(retryAssetsAction()).toEqual({ type: 'retryAssets' });
  });
  it('a choice sends no fit: the app layer keeps the node\'s fit, or uses cover', () => {
    expect(pickImageAction(base, asset)).toEqual({ type: 'setImageFill', id: 'n', assetId: ID });
    expect(pickImageAction(withImage, asset)).toEqual({ type: 'setImageFill', id: 'n', assetId: ID });
  });
  it('the fit control sends the image it has with the new fit, and nothing for a node with no image', () => {
    expect(fitAction(withImage, 'fill')).toEqual({ type: 'setImageFill', id: 'n', assetId: 'a'.repeat(64), fit: 'fill' });
    expect(fitAction(base, 'fill')).toBeNull();
  });
});

describe('the arrow keys of the grid', () => {
  it('move by one and by a row, and stop at the ends', () => {
    expect(gridMove('ArrowRight', 0, 7, 3)).toBe(1);
    expect(gridMove('ArrowRight', 6, 7, 3)).toBe(6);
    expect(gridMove('ArrowLeft', 0, 7, 3)).toBe(0);
    expect(gridMove('ArrowLeft', 4, 7, 3)).toBe(3);
    expect(gridMove('ArrowDown', 1, 7, 3)).toBe(4);
    expect(gridMove('ArrowDown', 5, 7, 3)).toBe(6);
    expect(gridMove('ArrowUp', 4, 7, 3)).toBe(1);
    expect(gridMove('ArrowUp', 1, 7, 3)).toBe(0);
    expect(gridMove('Home', 5, 7, 3)).toBe(0);
    expect(gridMove('End', 1, 7, 3)).toBe(6);
  });
  it('leaves the keys of the browser and the system alone while Alt, Ctrl or Meta is held', () => {
    expect(gridMove('ArrowLeft', 4, 7, 3, true)).toBeUndefined();
    expect(gridMove('Home', 4, 7, 3, true)).toBeUndefined();
    expect(gridMove('End', 4, 7, 3, false)).toBe(6);
  });
  it('ignores other keys and an empty grid', () => {
    expect(gridMove('a', 0, 7, 3)).toBeUndefined();
    expect(gridMove('ArrowRight', 0, 0, 3)).toBeUndefined();
  });
});

describe('where the focus goes as pages of images arrive', () => {
  it('goes to the first tile of the first page when it was parked on the picker for lack of a tile, and nowhere when it is elsewhere', () => {
    expect(followPages(FOLLOW_START, 3, false, true)).toEqual({ next: { seen: 3, following: false }, focus: 0 });
    expect(followPages(FOLLOW_START, 3, false, false)).toEqual({ next: { seen: 3, following: false }, focus: null });
  });
  it('does nothing while there is no tile, and keeps what it remembers', () => {
    expect(followPages(FOLLOW_START, 0, false, true)).toEqual({ next: FOLLOW_START, focus: null });
    expect(followPages(askedForMore({ seen: 24, following: false }), 24, false, false)).toEqual({ next: { seen: 24, following: true }, focus: null });
  });
  it('goes to the first new tile of a page that Load more asked for, once, and not for a page nobody asked for', () => {
    const asked = askedForMore({ seen: 24, following: false });
    expect(asked).toEqual({ seen: 24, following: true });
    expect(followPages(asked, 48, false, false)).toEqual({ next: { seen: 48, following: false }, focus: 24 });
    expect(followPages({ seen: 48, following: false }, 54, false, false)).toEqual({ next: { seen: 54, following: false }, focus: null });
  });
  it('forgets that a page was asked for when the list failed, so a later page does not take the focus', () => {
    const asked = askedForMore({ seen: 24, following: false });
    expect(followPages(asked, 24, true, false)).toEqual({ next: { seen: 24, following: false }, focus: null });
    expect(followPages({ seen: 24, following: false }, 48, false, false).focus).toBeNull();
  });
  it('is a function of its arguments: it changes nothing it was given', () => {
    const start = { seen: 0, following: false };
    followPages(start, 5, false, true);
    expect(start).toEqual({ seen: 0, following: false });
    expect(FOLLOW_START).toEqual({ seen: 0, following: false });
  });
});
