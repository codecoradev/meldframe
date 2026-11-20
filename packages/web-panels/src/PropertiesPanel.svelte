<!--
  The properties panel (the Design side of the right card): the values of the one selected node in the sections of the
  Properties and Inspector boards. A field reports its text when the person leaves it or presses Enter; what that
  becomes (an action, or nothing, and then the field shows the document's value again) is in properties.ts. It holds no
  state of its own.
-->
<script lang="ts">
  import type { Action, NodeKind, Properties } from '@meldframe/protocol';
  import Checkbox from '@meldframe/web-chrome/Checkbox.svelte';
  import Field from '@meldframe/web-chrome/Field.svelte';
  import Icon, { type IconName } from '@meldframe/web-chrome/Icon.svelte';
  import IconButton from '@meldframe/web-chrome/IconButton.svelte';
  import Section from '@meldframe/web-chrome/Section.svelte';
  import { bindingOf, boxAction, clipAction, fieldsOf, percent, propertyAction, unbindAction, type BoxField, type PropertyField } from '@meldframe/web-document/properties';

  interface Props {
    properties: Properties | null;
    /** How many nodes are selected: the empty state says which case it is. */
    selected: number;
    dispatch: (a: Action) => void;
  }
  let { properties, selected, dispatch }: Props = $props();

  const kindIcon: Record<NodeKind, IconName> = { frame: 'frame', rect: 'rectangle', ellipse: 'ellipse', text: 'text', shader: 'layers' };
  const round = (v: number) => String(Math.round(v * 100) / 100);
  const send = (a: Action | null) => {
    if (a) dispatch(a);
  };
  const prop = (p: Properties, f: PropertyField) => (raw: string) => send(propertyAction(p, f, raw));
  const box = (p: Properties, f: BoxField) => (raw: string) => send(boxAction(p, f, raw));
</script>

<div class="flex flex-col text-small text-text" data-testid="properties">
  {#if properties}
    {@const p = properties}
    {@const has = fieldsOf(p.kind)}
    {@const fill = bindingOf(p, 'fill')}
    {@const opacity = bindingOf(p, 'opacity')}
    {@const radius = bindingOf(p, 'radius')}
    {#key p.id}
      <header class="flex items-center gap-2 px-4 pt-4 text-text-muted">
        <Icon name={kindIcon[p.kind]} />
        <div class="min-w-0 flex-1"><Field label="Name" value={p.name} onCommit={prop(p, 'name')} testId="prop-name" /></div>
      </header>
      <Section title="Position">
        <div class="grid grid-cols-2 gap-2">
          <Field label="X" prefix="X" value={round(p.x)} onCommit={box(p, 'x')} testId="prop-x" />
          <Field label="Y" prefix="Y" value={round(p.y)} onCommit={box(p, 'y')} testId="prop-y" />
        </div>
      </Section>
      <Section title="Size">
        <div class="grid grid-cols-2 gap-2">
          <Field label="Width" prefix="W" suffix="px" value={round(p.w)} onCommit={box(p, 'w')} testId="prop-w" />
          <Field label="Height" prefix="H" suffix="px" value={round(p.h)} onCommit={box(p, 'h')} testId="prop-h" />
        </div>
      </Section>
      <Section title="Appearance">
        <div class="grid grid-cols-2 items-end gap-2">
          <div class="flex flex-col gap-2">
            <span class="flex min-w-0 gap-1 whitespace-nowrap text-text-muted">Opacity{#if opacity}<span class="truncate text-chip-text" title={opacity.name}>· {opacity.name}</span>{/if}</span>
            <Field label="Opacity" suffix="%" value={percent(p.opacity)} bound={!!opacity} onCommit={prop(p, 'opacity')} testId="prop-opacity" />
          </div>
          {#if has.radius}
            <div class="flex flex-col gap-2">
              <span class="flex min-w-0 gap-1 whitespace-nowrap text-text-muted">Corner radius{#if radius}<span class="truncate text-chip-text" title={radius.name}>· {radius.name}</span>{/if}</span>
              <Field label="Corner radius" value={round(p.radius)} bound={!!radius} onCommit={prop(p, 'radius')} testId="prop-radius" />
            </div>
          {/if}
        </div>
        {#if has.clip}
          <Checkbox label="Clip content" checked={p.clip} onChange={(c) => send(clipAction(p, c))} data-testid="prop-clip" />
        {/if}
      </Section>
      {#if has.fontSize}
        <Section title="Text">
          <Field label="Font size" suffix="px" value={round(p.fontSize)} onCommit={prop(p, 'fontSize')} testId="prop-fontSize" />
        </Section>
      {/if}
      <Section title="Fill">
        {#if fill}
          <div class="flex h-(--mf-size-control-lg) items-center gap-2 rounded-md bg-chip-bg px-3 text-chip-text" data-testid="prop-fill-bound">
            <i class="block size-4 shrink-0 rounded-sm border border-border" style:background={p.fill}></i>
            <span class="min-w-0 flex-1 truncate font-medium" title={fill.name}>{fill.name}</span>
            <span class="font-mono text-caption" data-testid="prop-fill">{p.fill}</span>
            <IconButton label="Detach {fill.name}" icon="close" size={24} onclick={() => send(unbindAction(p, 'fill'))} data-testid="prop-fill-detach" />
          </div>
        {:else}
          <Field label="Fill" value={p.fill} onCommit={prop(p, 'fill')} testId="prop-fill">
            {#snippet lead()}<i class="block size-4 shrink-0 rounded-sm border border-border" style:background={p.fill === 'none' ? 'transparent' : p.fill}></i>{/snippet}
          </Field>
        {/if}
      </Section>
    {/key}
  {:else}
    <div class="flex flex-col gap-1 px-4 py-4" data-testid="properties-empty">
      <h3 class="m-0 text-body font-semibold text-text-strong">{selected > 1 ? `${selected} nodes selected` : 'Nothing selected'}</h3>
      <p class="m-0 text-text-muted">{selected > 1 ? 'Select one node to edit its properties.' : 'Select a frame, a shape or text on the canvas to edit it.'}</p>
    </div>
  {/if}
</div>
