<!--
  The Fill section as the Properties, Inspector and Fill-and-layout boards draw it: the type switch (solid, linear,
  radial, none), then the fields of the type: a colour with the bind button (or the chip of the variable it is bound to),
  or a gradient with its angle, a bar, its stops and a button to add one. An image fill is shown, not picked: the app
  layer has no way to choose an asset yet. It draws the document and reports input as actions (see fills.ts).
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { Action, BindField, FieldBinding, Picker, Properties } from '@meldframe/protocol';
  import Button from '@meldframe/web-chrome/Button.svelte';
  import Field from '@meldframe/web-chrome/Field.svelte';
  import IconButton from '@meldframe/web-chrome/IconButton.svelte';
  import Section from '@meldframe/web-chrome/Section.svelte';
  import Segmented from '@meldframe/web-chrome/Segmented.svelte';
  import { addStopAction, angleAction, FILL_TYPES, fillTypeAction, gradientBar, removeStopAction, stopColorAction, stopPercent, stopPosAction } from '@meldframe/web-document/fills';
  import { bindingOf, propertyAction } from '@meldframe/web-document/properties';
  import BindPicker from './BindPicker.svelte';

  interface Props {
    properties: Properties;
    picker: Picker | null;
    dispatch: (a: Action) => void;
    /** The chip of a bound field, drawn by the panel so every bound field looks alike. */
    chip: Snippet<[string, BindField, FieldBinding, string]>;
  }
  let { properties: p, picker, dispatch, chip }: Props = $props();
  const send = (a: Action | null) => {
    if (a) dispatch(a);
  };
  const fill = $derived(bindingOf(p, 'fill'));
  // What the node can take is the app layer's to say; an image fill is not offered, but it is marked when the node has one.
  const types = $derived([...(p.gradientAllowed ? FILL_TYPES : FILL_TYPES.filter((t) => t.value === 'solid' || t.value === 'none')), ...(p.fillKind === 'image' ? [{ value: 'image' as const, label: 'Image' }] : [])]);
  const swatch = $derived(p.fill === 'none' ? 'transparent' : p.fill);
</script>

<Section title="Fill">
  <Segmented label="Fill type" options={types} value={p.fillKind} onChange={(k) => send(fillTypeAction(p, k as typeof p.fillKind))} data-testid="prop-fill-type" />
  {#if p.fillKind === 'solid' || p.fillKind === 'none' || p.fillKind === 'image'}
    {#if p.fillKind === 'image' && p.image}
      <p class="m-0 text-text-muted" data-testid="prop-fill-image">Image fill, {p.image.fit}. Choosing an image is not available here yet.</p>
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
          <BindPicker properties={p} field="fill" {picker} {dispatch} />
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
