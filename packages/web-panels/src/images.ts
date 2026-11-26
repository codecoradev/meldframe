// The image picker of the Fill section (E2.5, ADR-0015 follow-up, decision 36): what a tile says (assets have no names yet: the
// first 8 characters of the id, the type and the size), the action a click sends and where the arrow keys go. The rules are the
// app layer's (which kinds take an image, the fit a choice gets, what a retry asks for, how long the list may be); these
// functions turn a click into the action to send. Pure.
import type { Action, AssetInfo, FillKind, ImageFit, Properties } from '@meldframe/protocol';
import { FILL_TYPES } from '@meldframe/web-document/fills';

/** How the image fills the box, in the words of the file format: cover crops, contain keeps the whole image, fill stretches. */
export const IMAGE_FITS: { value: ImageFit; label: string }[] = [
  { value: 'cover', label: 'Cover' },
  { value: 'contain', label: 'Contain' },
  { value: 'fill', label: 'Fill' },
];

/** The types the switch offers: the Image type is offered where the app layer says the node can hold one (`imageAllowed`), before none. */
export function fillTypes(p: Properties): { value: FillKind; label: string }[] {
  const base = p.gradientAllowed ? FILL_TYPES : FILL_TYPES.filter((t) => t.value === 'solid' || t.value === 'none');
  if (!p.imageAllowed) return base;
  const none = base.findIndex((t) => t.value === 'none');
  return [...base.slice(0, none < 0 ? base.length : none), { value: 'image', label: 'Image' }, ...base.slice(none < 0 ? base.length : none)];
}

/** The first 8 characters of an asset id: what a tile and the current image show in place of a name. */
export const shortId = (id: string): string => id.slice(0, 8);

const TYPES: Record<string, string> = { 'image/png': 'PNG', 'image/jpeg': 'JPEG', 'image/webp': 'WebP', 'image/gif': 'GIF' };
/** The name of an image type for a person; a type the server does not serve is shown as it came. */
export const typeText = (mime: string): string => TYPES[mime] ?? mime;

/** A size for a person: bytes, then KB and MB (1,024 to the step) with one decimal for MB. */
export function sizeText(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return '';
  if (bytes < 1024) return `${Math.trunc(bytes)} B`;
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${(Math.round((bytes / (1024 * 1024)) * 10) / 10).toFixed(1)} MB`;
}

/** The name of a tile: what its label shows, then the type and the size. */
export const tileLabel = (a: AssetInfo): string => `Image ${shortId(a.id)}, ${typeText(a.mime)}, ${sizeText(a.bytes)}`;

export const openAssetsAction = (p: Properties): Action => ({ type: 'openAssets', id: p.id });
export const closeAssetsAction = (): Action => ({ type: 'closeAssets' });
export const loadMoreAction = (): Action => ({ type: 'loadMoreAssets' });
/** Try the request that failed again; the app layer decides whether that is the first page or the next one. */
export const retryAssetsAction = (): Action => ({ type: 'retryAssets' });

/** A tile was chosen: the image goes on the node. It sends no fit, so the app layer keeps the one the node has, or uses cover, and closes the picker. */
export const pickImageAction = (p: Properties, a: AssetInfo): Action => ({ type: 'setImageFill', id: p.id, assetId: a.id });

/** The fit control: the image the node has with the new fit; nothing for a node with no image (the control is not drawn then). */
export function fitAction(p: Properties, fit: ImageFit): Action | null {
  return p.image ? { type: 'setImageFill', id: p.id, assetId: p.image.assetId, fit } : null;
}

/** What the picker remembers between draws to know where the focus goes as pages of images arrive. */
export interface Follow {
  /** How many tiles the last draw had. */
  seen: number;
  /** Load more was pressed and its page has not come. */
  following: boolean;
}
export const FOLLOW_START: Follow = { seen: 0, following: false };

/** Load more was pressed: the first new tile of the page that comes takes the focus. */
export const askedForMore = (f: Follow): Follow => ({ ...f, following: true });

/**
 * Where the focus goes as the list grows, asked after each draw with the number of tiles, whether the list failed and whether the
 * focus is on the picker itself (it was parked there because there was no tile to take it). The first page, when the focus was parked:
 * the first tile. A page asked for with Load more: its first new tile (the button may be gone with the last page). A failed list
 * forgets that a page was asked for. Answers what to remember and the tile to focus, null for none; it changes nothing it was given.
 */
export function followPages(f: Follow, count: number, error: boolean, parked: boolean): { next: Follow; focus: number | null } {
  const following = f.following && !error;
  const first = f.seen === 0 && parked ? 0 : following && count > f.seen ? f.seen : -1;
  const moves = count > f.seen && first >= 0;
  return { next: { seen: count, following: moves ? false : following }, focus: moves ? first : null };
}

/**
 * Where an arrow key takes the cursor of a grid of `count` tiles in `cols` columns: one tile along or one row up or down, and
 * Home and End to the first and the last tile of the whole list (not of a row). Undefined for any other key, for an empty grid
 * and while Alt, Ctrl or Meta is held, so the browser's and the system's shortcuts (Alt+Left to go back) are left alone.
 */
export function gridMove(key: string, index: number, count: number, cols: number, modified = false): number | undefined {
  if (count <= 0 || modified) return undefined;
  const last = count - 1;
  switch (key) {
    case 'ArrowRight': return Math.min(last, index + 1);
    case 'ArrowLeft': return Math.max(0, index - 1);
    case 'ArrowDown': return Math.min(last, index + cols);
    case 'ArrowUp': return Math.max(0, index - cols);
    case 'Home': return 0;
    case 'End': return last;
    default: return undefined;
  }
}
