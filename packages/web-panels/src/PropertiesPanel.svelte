<!--
  The properties panel (the Design side of the right card): the values of the one selected node in the sections of the
  Properties and Inspector boards. A field reports its text when the person leaves it or presses Enter; what that
  becomes (an action, or nothing, and then the field shows the document's value again) is in properties.ts. It holds no
  state of its own.
-->
<script lang="ts">
  import type { Component, ComponentProps } from 'svelte';
  import type { Action, AssetsState, BindField, FieldBinding, NodeKind, Picker, Properties } from '@meldframe/protocol';
  import Checkbox from '@meldframe/web-chrome/Checkbox.svelte';
  import Field from '@meldframe/web-chrome/Field.svelte';
  import Icon, { type IconName } from '@meldframe/web-chrome/Icon.svelte';
  import IconButton from '@meldframe/web-chrome/IconButton.svelte';
  import Section from '@meldframe/web-chrome/Section.svelte';
  import { bindingOf, clipAction, percent, textAction, unbindAction, type PlainField } from '@meldframe/web-document/properties';
  import BindPicker from './BindPicker.svelte';
  import BorderSection from './BorderSection.svelte';
  import FillSection from './FillSection.svelte';
  import LayoutSection from './LayoutSection.svelte';
  import PlacementSection from './PlacementSection.svelte';
  import ShadowSection from './ShadowSection.svelte';
  import SizeModes from './SizeModes.svelte';
  import TextStyle from './TextStyle.svelte';

  interface Props {
    properties: Properties | null;
    /** The bind picker of the app state, open on at most one field. */
    picker?: Picker | null;
    /** The image picker of the app state (open for at most one node) and the images it has read. */
    assets?: AssetsState;
    /** How many nodes are selected: the empty state says which case it is. */
    selected: number;
    dispatch: (a: Action) => void;
    /** The bind picker as the page draws it: the Svelte one unless the page gives another (the React pilot, `/?react=variables`). */
    bindView?: Component<ComponentProps<typeof BindPicker>>;
  }
  let { properties, picker = null, assets, selected, dispatch, bindView: BindView = BindPicker }: Props = $props();

  const kindIcon: Record<NodeKind, IconName> = { frame: 'frame', rect: 'rectangle', ellipse: 'ellipse', text: 'text', shader: 'layers' };
  const round = (v: number) => String(Math.round(v * 100) / 100);
  const send = (a: Action | null) => {
    if (a) dispatch(a);
  };
  // What is typed goes to the app layer as typed: it parses, holds the value to the range of the field and sends nothing when nothing changes.
  const text = (p: Properties, field: PlainField['type']) => (raw: string) => send(textAction(p, { type: field }, raw));
</script>

{#snippet chip(label: string, field: BindField, b: FieldBinding, value: string)}
  <div class="flex flex-col gap-2" data-testid="prop-{field}-bound">
    {#if label}<span class="text-text-muted">{label}</span>{/if}
    <div class="flex h-(--mf-size-control-lg) items-center gap-2 rounded-md px-3 {b.missing ? 'bg-danger-bg text-danger-text' : 'bg-chip-bg text-chip-text'}">
      {#if field === 'fill'}<i class="block size-4 shrink-0 rounded-sm border border-border" style:background={value}></i>{/if}
      <span class="min-w-0 flex-1 truncate font-medium" title={b.missing ? `${b.name} (variable removed)` : b.name}>{b.name}{b.missing ? ' (removed)' : ''}</span>
      <span class="font-mono text-caption" data-testid={field === 'fill' ? 'prop-fill' : `prop-${field}`}>{value}</span>
      <IconButton label="Detach {b.name}" icon="close" size={24} onclick={() => send(unbindAction(properties!, field))} data-testid="prop-{field}-detach" />
    </div>
  </div>
{/snippet}

<div class="flex flex-col border-t border-hairline text-small text-text" role="group" aria-label="Properties" data-testid="properties">
  {#if properties}
    {@const p = properties}
    {@const has = p.sections}
    {@const opacity = bindingOf(p, 'opacity')}
    {@const radius = bindingOf(p, 'radius')}
    {#key p.id}
      <header class="flex items-center gap-2 px-4 pt-4 text-text-muted">
        <Icon name={kindIcon[p.kind]} />
        <div class="min-w-0 flex-1"><Field label="Name" value={p.name} onCommit={text(p, 'name')} testId="prop-name" /></div>
      </header>
      <Section title="Position">
        <div class="grid grid-cols-2 gap-2">
          <Field label="X" prefix="X" mono value={round(p.x)} onCommit={text(p, 'x')} testId="prop-x" disabled={p.positionLocked} describedBy={p.positionLocked ? 'prop-position-note' : undefined} />
          <Field label="Y" prefix="Y" mono value={round(p.y)} onCommit={text(p, 'y')} testId="prop-y" disabled={p.positionLocked} describedBy={p.positionLocked ? 'prop-position-note' : undefined} />
        </div>
        {#if p.positionLocked}
          <!-- The position is the layout's: the fields show where the node is drawn and cannot move it. -->
          <p id="prop-position-note" class="m-0 text-caption text-text-muted">Set by the auto layout of the parent.</p>
        {/if}
      </Section>
      <Section title="Size">
        <div class="grid grid-cols-2 gap-2">
          <Field label="Width" prefix="W" suffix="px" mono value={round(p.w)} onCommit={text(p, 'w')} testId="prop-w" />
          <Field label="Height" prefix="H" suffix="px" mono value={round(p.h)} onCommit={text(p, 'h')} testId="prop-h" />
        </div>
        <SizeModes properties={p} {dispatch} />
      </Section>
      {#if has.layout}
        <LayoutSection properties={p} {dispatch} />
      {/if}
      {#if has.placement}
        <PlacementSection properties={p} {dispatch} />
      {/if}
      <Section title="Appearance">
        <div class="grid grid-cols-2 items-end gap-2">
          {#if opacity}
            <div class="col-span-2">{@render chip('Opacity', 'opacity', opacity, percent(p.opacity) + ' %')}</div>
          {:else}
            <div class="flex flex-col gap-2">
              <span class="flex items-center justify-between text-text-muted">Opacity<BindView properties={p} field="opacity" {picker} {dispatch} /></span>
              <Field label="Opacity" suffix="%" value={percent(p.opacity)} onCommit={text(p, 'opacity')} testId="prop-opacity" />
            </div>
          {/if}
          {#if has.radius}
            {#if radius}
              <div class="col-span-2">{@render chip('Corner radius', 'radius', radius, round(p.radius))}</div>
            {:else}
              <div class="flex flex-col gap-2">
                <span class="flex items-center justify-between text-text-muted">Corner radius<BindView properties={p} field="radius" {picker} {dispatch} /></span>
                <Field label="Corner radius" value={round(p.radius)} onCommit={text(p, 'radius')} testId="prop-radius" />
              </div>
            {/if}
          {/if}
        </div>
        {#if has.clip}
          <Checkbox label="Clip content" checked={p.clip} onChange={(c) => send(clipAction(p, c))} data-testid="prop-clip" />
        {/if}
      </Section>
      {#if has.fontSize}
        <Section title="Text">
          <Field label="Font size" suffix="px" value={round(p.fontSize)} onCommit={text(p, 'fontSize')} testId="prop-fontSize" />
          <TextStyle properties={p} {dispatch} />
        </Section>
      {/if}
      <FillSection properties={p} {picker} {assets} {dispatch} {chip} bindView={BindView} />
      <BorderSection properties={p} {dispatch} />
      <ShadowSection properties={p} {dispatch} />
    {/key}
  {:else}
    <div class="flex flex-col gap-1 px-4 py-4" data-testid="properties-empty">
      <h3 class="m-0 text-body font-semibold text-text-strong">{selected > 1 ? `${selected} nodes selected` : 'Nothing selected'}</h3>
      <p class="m-0 text-text-muted">{selected > 1 ? 'Select one node to edit its properties.' : 'Select a frame, a shape or text on the canvas to edit it.'}</p>
    </div>
  {/if}
</div>
