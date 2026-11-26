<!--
  The Fill section as the Properties, Inspector and Fill-and-layout boards draw it: the type switch (solid, linear,
  radial, image, none), then the fields of the type: a colour with the bind button (or the chip of the variable it is bound to),
  or a gradient with its angle, a bar, its stops and a button to add one. The Image type opens the image picker (the
  workspace's images, ImageFillPicker; open while the app state says so); an image fill shows its thumbnail and short id,
  a button to change it and its fit. It draws the document and reports input as actions (see fills.ts and images.ts).
-->
<script lang="ts">
  import { tick, type Component, type ComponentProps, type Snippet } from 'svelte';
  import type { Action, AssetsState, BindField, FieldBinding, ImageFit, Picker, Properties } from '@meldframe/protocol';
  import { assetUrl, isAssetId } from '@meldframe/web-assets/assets';
  import Button from '@meldframe/web-chrome/Button.svelte';
  import Field from '@meldframe/web-chrome/Field.svelte';
  import IconButton from '@meldframe/web-chrome/IconButton.svelte';
  import Section from '@meldframe/web-chrome/Section.svelte';
  import Segmented from '@meldframe/web-chrome/Segmented.svelte';
  import { addStopAction, angleAction, fillTypeAction, gradientBar, removeStopAction, stopColorAction, stopPercent, stopPosAction } from '@meldframe/web-document/fills';
  import { bindingOf, propertyAction } from '@meldframe/web-document/properties';
  import BindPicker from './BindPicker.svelte';
  import ImageFillPicker from './ImageFillPicker.svelte';
  import { fillTypes, fitAction, IMAGE_FITS, openAssetsAction, shortId } from './images';

  const CLOSED: AssetsState = { items: [], loading: false, loaded: false, capped: false, request: 0 };

  interface Props {
    properties: Properties;
    picker: Picker | null;
    /** The image picker of the app state: open for the node it names, with the images read so far. */
    assets?: AssetsState;
    dispatch: (a: Action) => void;
    /** The chip of a bound field, drawn by the panel so every bound field looks alike. */
    chip: Snippet<[string, BindField, FieldBinding, string]>;
    /** The bind picker as the page draws it: the Svelte one unless the page gives another (the React pilot, `/?react=variables`). */
    bindView?: Component<ComponentProps<typeof BindPicker>>;
  }
  let { properties: p, picker, assets = CLOSED, dispatch, chip, bindView: BindView = BindPicker }: Props = $props();
  const send = (a: Action | null) => {
    if (a) dispatch(a);
  };
  const fill = $derived(bindingOf(p, 'fill'));
  // What the node can take is the app layer's to say (gradientAllowed, imageAllowed).
  const types = $derived(fillTypes(p));
  const open = $derived(assets.target === p.id);
  const choose = (k: string) => send(k === 'image' ? openAssetsAction(p) : fillTypeAction(p, k as typeof p.fillKind));
  // The picker closed for this node (a choice, Escape or its button) and focus fell to the page: put it back on the switch.
  // A picker that closed because another node was selected, or because the person clicked somewhere else, leaves focus alone.
  let host: HTMLElement | undefined = $state();
  let was: string | null = null;
  $effect(() => {
    const now = open ? p.id : null;
    const closed = was === p.id && now === null;
    was = now;
    if (!closed) return;
    void tick().then(() => {
      if (document.activeElement && document.activeElement !== document.body) return;
      host?.querySelector<HTMLElement>('[role=radio][aria-checked=true]')?.focus();
    });
  });
  const swatch = $derived(p.fill === 'none' ? 'transparent' : p.fill);
</script>

<Section title="Fill">
  <div bind:this={host} class="contents">
    <Segmented label="Fill type" options={types} value={p.fillKind} onChange={choose} data-testid="prop-fill-type" />
  </div>
  {#if open}
    <ImageFillPicker properties={p} {assets} {dispatch} />
  {/if}
  {#if p.fillKind === 'solid' || p.fillKind === 'none' || p.fillKind === 'image'}
    {#if p.fillKind === 'image' && p.image}
      <div class="flex items-center gap-2" data-testid="prop-fill-image">
        <span class="block size-8 shrink-0 overflow-hidden rounded-sm border border-border bg-field">{#if isAssetId(p.image.assetId)}<img src={assetUrl(p.image.assetId)} alt="" decoding="async" class="size-full object-contain" />{/if}</span>
        <span class="min-w-0 flex-1 truncate font-mono text-caption" title={p.image.assetId}>Image {shortId(p.image.assetId)}</span>
        <Button size="sm" onclick={() => send(openAssetsAction(p))} data-testid="prop-fill-image-change">Change image</Button>
      </div>
      <Segmented label="Image fit" options={IMAGE_FITS} value={p.image.fit} onChange={(f) => send(fitAction(p, f as ImageFit))} data-testid="prop-fill-image-fit" />
    {/if}
    {#if p.fillKind === 'solid'}
      {#if fill}
        {@render chip('', 'fill', fill, p.fill)}
      {:else}
        <div class="flex items-center gap-2">
          <div class="min-w-0 flex-1">
            <Field label="Fill" value={p.fill} onCommit={(raw) => send(propertyAction(p, 'fill', raw))} testId="prop-fill">
              {#snippet lead()}<i class="block size-4 shrink-0 rounded-sm border border-border" style:background={swatch}></i>{/snippet}
            </Field>
          </div>
          <BindView properties={p} field="fill" {picker} {dispatch} />
        </div>
      {/if}
    {/if}
  {:else if p.gradient}
    {@const g = p.gradient}
    {#if p.fillKind === 'linear'}
      <div class="flex items-center gap-2">
        <span class="w-10 text-text-muted">Angle</span>
        <div class="w-24"><Field label="Angle" suffix="°" mono value={String(Math.round(g.angle * 100) / 100)} onCommit={(raw) => send(angleAction(p, raw))} testId="prop-gradient-angle" /></div>
      </div>
    {/if}
    <div class="h-4 rounded-sm border border-border" style:background={gradientBar(g)} data-testid="prop-gradient-bar" role="img" aria-label="Gradient, {g.stops.length} stops"></div>
    <div class="flex flex-col gap-1">
      {#each g.stops as s, i (i)}
        <div class="grid grid-cols-[auto_minmax(0,1fr)_auto_auto] items-center gap-2" data-testid="prop-gradient-stop">
          <i class="block size-4 shrink-0 rounded-sm border border-border" style:background={s.color}></i>
          <Field label="Stop {i + 1} colour" size="dense" mono value={s.color} onCommit={(raw) => send(stopColorAction(p, i, raw))} testId="prop-gradient-color" />
          <div class="w-24"><Field label="Stop {i + 1} position" size="dense" mono suffix="%" value={stopPercent(s.pos)} onCommit={(raw) => send(stopPosAction(p, i, raw))} testId="prop-gradient-pos" /></div>
          <IconButton label="Remove stop {i + 1}" icon="minus" size={24} disabled={g.stops.length <= p.minStops} onclick={() => send(removeStopAction(p, i))} data-testid="prop-gradient-remove" />
        </div>
      {/each}
    </div>
    <div class="flex"><Button size="sm" disabled={g.stops.length >= p.maxStops} onclick={() => send(addStopAction(p))} data-testid="prop-gradient-add">Add stop</Button></div>
  {/if}
</Section>
