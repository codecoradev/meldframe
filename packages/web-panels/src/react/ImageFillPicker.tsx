// React copy of ImageFillPicker.svelte. The image picker of the Fill section: the images the workspace holds, as a grid of tiles with a
// thumbnail and the first 8 characters of the id (an asset has no name yet), Load more when the server has more, Retry previews when
// thumbnails did not load, and notes for an empty workspace, a list that is loading and one that failed. The tile of the image the node
// has carries `aria-current` (a press chooses and closes, so it is not a toggle). A tile sets the image of the node (the app layer
// closes the picker); Escape and the close button close it. There is no upload: an agent uploads. Whether it is open, the list and the
// cursor are the app state's; only the keyboard cursor of the grid and the failed previews are local.
//
// It is a card inside the Fill section, not an overlay: nothing is portalled, and no Base UI family is involved. The section draws it
// while the app state says it is open for the node, so it is made again (an empty set of failed previews, the focus on the first tile)
// each time it opens.
//
// Keyboard: the grid is one tab stop (the tile under the cursor); the arrow keys, Home and End move the cursor and focus by one tile and
// by a row (Home and End go to the first and the last tile read, not of a row), unless Alt, Ctrl or Meta is held; Enter or Space chooses.
// Focus moves to the first tile when the picker opens (or to the picker when there is none yet), stays on Load more while the next page
// comes and then moves to the first new tile (`followPages`). Where focus goes when the picker closes is the Fill section's.
//
// The route of a thumbnail is the page's (`thumbnail`): a view package may not import web-assets. An id the page has no route for gets
// a tile that says there is no preview, which is not counted as a failed load.
import type { Action, AssetsState, Properties } from '@meldframe/protocol';
import { Button } from '@meldframe/web-chrome/react/Button';
import { IconButton } from '@meldframe/web-chrome/react/IconButton';
import { ring } from '@meldframe/web-chrome/focus';
import { type KeyboardEvent, useEffect, useRef, useState } from 'react';
import { FOLLOW_START, askedForMore, closeAssetsAction, followPages, gridMove, loadMoreAction, pickImageAction, retryAssetsAction, shortId, tileLabel } from '../images';

/** Tiles to a row: the grid below has three columns. */
const COLS = 3;

const tilesOf = (root: HTMLElement | null): HTMLElement[] => [...(root?.querySelectorAll<HTMLElement>('[data-testid="image-tile"]') ?? [])];

export interface ImageFillPickerProps {
  properties: Properties;
  assets: AssetsState;
  dispatch: (a: Action) => void;
  /** Where the thumbnail of an asset is fetched from, or null for an id that has no route (see the header). */
  thumbnail: (assetId: string) => string | null;
}

export function ImageFillPicker({ properties: p, assets, dispatch, thumbnail }: ImageFillPickerProps) {
  const root = useRef<HTMLDivElement>(null);
  const [cursor, setCursor] = useState(0);
  // Previews that did not load (a file that is gone or cut short, or a busy server): the tile says so instead of drawing a broken image,
  // and Retry previews asks for them again.
  const [broken, setBroken] = useState<ReadonlySet<string>>(() => new Set());
  const count = assets.items.length;
  const active = Math.min(cursor, Math.max(0, count - 1));

  // A button that goes away with the notice it is in: focus goes to the tile under the cursor (the picker itself when there is none),
  // so Escape still closes the picker. It is done after the draw that took the button away. The app layer decides what a retry asks
  // for, so there is one action.
  const settling = useRef(false);
  useEffect(() => {
    if (!settling.current) return;
    settling.current = false;
    (tilesOf(root.current)[active] ?? root.current)?.focus();
  });
  const retry = () => {
    settling.current = true;
    dispatch(retryAssetsAction());
  };
  const retryPreviews = () => {
    settling.current = true;
    setBroken(new Set());
  };

  // Focus into the picker when it opens. Declared before the effect that follows the pages, so that one finds the focus on a tile.
  useEffect(() => {
    (tilesOf(root.current)[0] ?? root.current)?.focus();
  }, []);

  // Focus follows the pages: the rule is `followPages`, shared with the Svelte copy.
  const follow = useRef(FOLLOW_START);
  const failed = !!assets.error;
  useEffect(() => {
    const r = followPages(follow.current, count, failed, document.activeElement === root.current);
    follow.current = r.next;
    if (r.focus === null) return;
    setCursor(r.focus);
    tilesOf(root.current)[r.focus]?.focus();
  }, [count, failed]);

  const key = (e: KeyboardEvent, i: number) => {
    const to = gridMove(e.key, i, count, COLS, e.altKey || e.ctrlKey || e.metaKey);
    if (to === undefined) return;
    e.preventDefault();
    setCursor(to);
    tilesOf(root.current)[to]?.focus();
  };

  return (
    // Escape closes the picker from anywhere inside it; every control in it is a button with its own keys. It is taken in the capture phase:
    // with the focus on Close and its tooltip open, Base UI's tooltip would stop the key at its trigger and the first Escape would only close
    // the tooltip (the Svelte copy closes the picker at once; the same was found for the shortcut sheet in S4).
    <div
      ref={root}
      role="group"
      aria-labelledby="image-picker-title"
      tabIndex={-1}
      data-testid="image-picker"
      onKeyDownCapture={(e) => {
        if (e.key !== 'Escape') return;
        e.preventDefault();
        e.stopPropagation();
        dispatch(closeAssetsAction());
      }}
      className="flex flex-col gap-2 rounded-md border border-border bg-surface-raised p-2 text-small text-text"
    >
      <div className="flex items-center gap-2">
        <h4 id="image-picker-title" className="m-0 min-w-0 flex-1 text-small font-semibold text-text-strong">
          Choose an image
        </h4>
        <IconButton label="Close image picker" icon="close" size={24} onClick={() => dispatch(closeAssetsAction())} data-testid="image-picker-close" />
      </div>
      {assets.error ? (
        <div role="alert" className="flex flex-col gap-2 text-danger-text" data-testid="image-picker-error">
          <p className="m-0">{`Images could not be loaded (${assets.error}).`}</p>
          <div>
            <Button size="sm" onClick={retry} data-testid="image-picker-retry">
              Try again
            </Button>
          </div>
        </div>
      ) : assets.loading && !assets.loaded && count === 0 ? (
        <p className="m-0 text-text-muted" role="status" data-testid="image-picker-loading">
          Loading images…
        </p>
      ) : assets.loaded && count === 0 ? (
        <p className="m-0 text-text-muted" data-testid="image-picker-empty">
          No images in this workspace yet. An agent can upload one.
        </p>
      ) : null}
      {count > 0 ? (
        <ul className="m-0 grid max-h-[calc(var(--mf-size-control-lg)*8)] list-none grid-cols-3 gap-2 overflow-y-auto p-1" aria-label="Images in this workspace">
          {assets.items.map((a, i) => {
            const current = p.image?.assetId === a.id;
            const src = thumbnail(a.id);
            return (
              <li key={a.id} className="min-w-0">
                <button
                  type="button"
                  tabIndex={i === active ? 0 : -1}
                  aria-current={current ? 'true' : undefined}
                  aria-label={tileLabel(a)}
                  data-testid="image-tile"
                  data-id={a.id}
                  onClick={() => dispatch(pickImageAction(p, a))}
                  onKeyDown={(e) => key(e, i)}
                  onFocus={() => setCursor(i)}
                  className={`${ring} flex w-full flex-col gap-1 rounded-sm border p-1 text-left hov:bg-control ${current ? 'border-accent bg-chip-bg' : 'border-border'}`}
                >
                  <span className="relative block aspect-square w-full overflow-hidden rounded-sm bg-field">
                    {src === null || broken.has(a.id) ? (
                      <span className="absolute inset-0 grid place-items-center px-1 text-center text-caption text-text-muted">No preview</span>
                    ) : (
                      <img src={src} alt="" loading="lazy" decoding="async" onError={() => setBroken((was) => new Set(was).add(a.id))} className="size-full object-contain" />
                    )}
                  </span>
                  <span className="truncate font-mono text-caption text-text-muted">{shortId(a.id)}</span>
                </button>
              </li>
            );
          })}
        </ul>
      ) : null}
      {broken.size > 0 ? (
        <div className="flex items-center gap-2" data-testid="image-picker-broken">
          <p className="m-0 min-w-0 flex-1 text-caption text-text-muted" role="status">
            {`${broken.size === 1 ? '1 preview' : `${broken.size} previews`} did not load.`}
          </p>
          <Button size="sm" onClick={retryPreviews} data-testid="image-picker-retry-previews">
            Retry previews
          </Button>
        </div>
      ) : null}
      {assets.capped ? (
        <p className="m-0 text-caption text-text-muted" role="status" data-testid="image-picker-capped">
          {`The first ${count} images are listed; there are more in this workspace.`}
        </p>
      ) : null}
      {assets.next ? (
        <div className="flex">
          <Button
            size="sm"
            onClick={() => {
              follow.current = askedForMore(follow.current);
              dispatch(loadMoreAction());
            }}
            data-testid="image-picker-more"
          >
            {assets.loading ? 'Loading…' : 'Load more'}
          </Button>
        </div>
      ) : null}
    </div>
  );
}
