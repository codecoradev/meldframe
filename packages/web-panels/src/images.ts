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
