<!--
  The image picker of the Fill section: the images the workspace holds, as a grid of tiles with a thumbnail and the first 8
  characters of the id (an asset has no name yet), Load more when the server has more, Retry previews when thumbnails did
  not load, and notes for an empty workspace, a list that is loading and one that failed. The tile of the image the node has
  carries `aria-current` (a press chooses and closes, so it is not a toggle). A tile sets the image of the node (the app layer
  closes the picker); Escape and the close button close it. There is no upload: an agent uploads. Whether it is open, the
  list and the cursor are the app state's; only the keyboard cursor of the grid and the failed previews are local.

  Keyboard: the grid is one tab stop (the tile under the cursor); the arrow keys, Home and End move the cursor and focus
  by one tile and by a row (Home and End go to the first and the last tile read, not of a row), unless Alt, Ctrl or Meta is
  held; Enter or Space chooses. Focus moves to the first tile when the picker opens (or to the picker
  when there is none yet), stays on Load more while the next page comes and then moves to the first new tile. Where focus
  goes when the picker closes is the Fill section's (it knows the node and the switch).
-->
<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { SvelteSet } from 'svelte/reactivity';
  import type { Action, AssetsState, Properties } from '@meldframe/protocol';
  import { assetUrl } from '@meldframe/web-assets/assets';
  import Button from '@meldframe/web-chrome/Button.svelte';
  import IconButton from '@meldframe/web-chrome/IconButton.svelte';
  import { ring } from '@meldframe/web-chrome/focus';
  import { closeAssetsAction, gridMove, loadMoreAction, pickImageAction, retryAssetsAction, shortId, tileLabel } from './images';

  interface Props {
    properties: Properties;
    assets: AssetsState;
    dispatch: (a: Action) => void;
  }
  let { properties: p, assets, dispatch }: Props = $props();

  /** Tiles to a row: the grid below has three columns. */
  const COLS = 3;
  let root: HTMLElement | undefined = $state();
  let cursor = $state(0);
  /** Previews that did not load (a file that is gone or cut short, or a busy server): the tile says so instead of drawing a broken
   *  image, and Retry previews asks for them again. The set starts empty each time the picker opens (it is made again then). */
  const broken = new SvelteSet<string>();
  const active = $derived(Math.min(cursor, Math.max(0, assets.items.length - 1)));
  const tiles = (): HTMLElement[] => [...(root?.querySelectorAll<HTMLElement>('[data-testid="image-tile"]') ?? [])];
  // A button that goes away with the notice it is in: focus goes to the tile under the cursor (the picker itself when there is
  // none), so Escape still closes the picker. The app layer decides what a retry asks for, so there is one action.
  const settle = () => void tick().then(() => (tiles()[active] ?? root)?.focus());
  const retry = () => {
    dispatch(retryAssetsAction());
    settle();
  };
  const retryPreviews = () => {
    broken.clear();
    settle();
  };

  onMount(() => {
    (tiles()[0] ?? root)?.focus();
  });

  // Focus follows the pages. The first page, when focus was parked on the picker because there was no tile to take it: to
  // the first tile. A page asked for with Load more: to its first new tile (the button may be gone with the last page).
  let seen = 0;
  let following = false;
  $effect(() => {
    const n = assets.items.length;
    if (assets.error) following = false;
    const first = seen === 0 && document.activeElement === root ? 0 : following && n > seen ? seen : -1;
    if (n > seen && first >= 0) {
      following = false;
      cursor = first;
      void tick().then(() => tiles()[first]?.focus());
    }
    seen = n;
  });

  function key(e: KeyboardEvent, i: number) {
    const to = gridMove(e.key, i, assets.items.length, COLS, e.altKey || e.ctrlKey || e.metaKey);
    if (to === undefined) return;
    e.preventDefault();
    cursor = to;
    tiles()[to]?.focus();
  }
</script>

<!-- Escape closes the picker from anywhere inside it; every control in it is a button with its own keys. -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  bind:this={root}
  role="group"
  aria-labelledby="image-picker-title"
  tabindex="-1"
  data-testid="image-picker"
  onkeydown={(e) => {
    if (e.key !== 'Escape') return;
    e.preventDefault();
    e.stopPropagation();
    dispatch(closeAssetsAction());
  }}
  class="flex flex-col gap-2 rounded-md border border-border bg-surface-raised p-2 text-small text-text"
>
  <div class="flex items-center gap-2">
    <h4 id="image-picker-title" class="m-0 min-w-0 flex-1 text-small font-semibold text-text-strong">Choose an image</h4>
    <IconButton label="Close image picker" icon="close" size={24} onclick={() => dispatch(closeAssetsAction())} data-testid="image-picker-close" />
  </div>
  {#if assets.error}
    <div role="alert" class="flex flex-col gap-2 text-danger-text" data-testid="image-picker-error">
      <p class="m-0">Images could not be loaded ({assets.error}).</p>
      <div><Button size="sm" onclick={retry} data-testid="image-picker-retry">Try again</Button></div>
    </div>
  {:else if assets.loading && !assets.loaded && assets.items.length === 0}
    <p class="m-0 text-text-muted" role="status" data-testid="image-picker-loading">Loading images…</p>
  {:else if assets.loaded && assets.items.length === 0}
    <p class="m-0 text-text-muted" data-testid="image-picker-empty">No images in this workspace yet. An agent can upload one.</p>
  {/if}
  {#if assets.items.length > 0}
    <ul class="m-0 grid max-h-[calc(var(--mf-size-control-lg)*8)] list-none grid-cols-3 gap-2 overflow-y-auto p-1" aria-label="Images in this workspace">
      {#each assets.items as a, i (a.id)}
        {@const current = p.image?.assetId === a.id}
        <li class="min-w-0">
          <button
            type="button"
            tabindex={i === active ? 0 : -1}
            aria-current={current ? 'true' : undefined}
            aria-label={tileLabel(a)}
            data-testid="image-tile"
            data-id={a.id}
            onclick={() => dispatch(pickImageAction(p, a))}
            onkeydown={(e) => key(e, i)}
            onfocus={() => (cursor = i)}
            class="{ring} flex w-full flex-col gap-1 rounded-sm border p-1 text-left hov:bg-control {current ? 'border-accent bg-chip-bg' : 'border-border'}"
          >
            <span class="relative block aspect-square w-full overflow-hidden rounded-sm bg-field">
              {#if broken.has(a.id)}
                <span class="absolute inset-0 grid place-items-center px-1 text-center text-caption text-text-muted">No preview</span>
              {:else}
                <img src={assetUrl(a.id)} alt="" loading="lazy" decoding="async" onerror={() => broken.add(a.id)} class="size-full object-contain" />
              {/if}
            </span>
            <span class="truncate font-mono text-caption text-text-muted">{shortId(a.id)}</span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
  {#if broken.size > 0}
    <div class="flex items-center gap-2" data-testid="image-picker-broken">
      <p class="m-0 min-w-0 flex-1 text-caption text-text-muted" role="status">{broken.size === 1 ? '1 preview' : `${broken.size} previews`} did not load.</p>
      <Button size="sm" onclick={retryPreviews} data-testid="image-picker-retry-previews">Retry previews</Button>
    </div>
  {/if}
  {#if assets.capped}
    <p class="m-0 text-caption text-text-muted" role="status" data-testid="image-picker-capped">The first {assets.items.length} images are listed; there are more in this workspace.</p>
  {/if}
  {#if assets.next}
    <div class="flex">
      <Button size="sm" onclick={() => { following = true; dispatch(loadMoreAction()); }} data-testid="image-picker-more">{assets.loading ? 'Loading…' : 'Load more'}</Button>
    </div>
  {/if}
</div>
