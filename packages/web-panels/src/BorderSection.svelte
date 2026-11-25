<!--
  The Border section as the Properties and Fill-and-layout boards draw it: the width, the colour with its swatch, and
  where the border sits against the edge, and while there is a border a Remove button (the width 0 the field sends). A
  width of 0 is no border. It draws the document and reports input as actions
  (see decoration.ts); it holds no state of its own.
-->
<script lang="ts">
  import type { Action, Properties } from '@meldframe/protocol';
  import IconButton from '@meldframe/web-chrome/IconButton.svelte';
  import Field from '@meldframe/web-chrome/Field.svelte';
  import Section from '@meldframe/web-chrome/Section.svelte';
  import Segmented from '@meldframe/web-chrome/Segmented.svelte';
  import { BORDER_POSITIONS, borderPositionAction, removeBorderAction } from '@meldframe/web-document/decoration';
  import { textAction } from '@meldframe/web-document/properties';

  interface Props {
    properties: Properties;
    dispatch: (a: Action) => void;
  }
  let { properties: p, dispatch }: Props = $props();
  const send = (a: Action | null) => {
    if (a) dispatch(a);
  };
  const on = $derived(p.border.width > 0);
  // The button goes away with the border, so focus moves first, to the width field (which stays and is what the button
  // changes), and the action is sent after: focus never falls to the page, whenever the document answers.
  let widths: HTMLElement | undefined = $state();
  const remove = () => {
    widths?.querySelector<HTMLElement>('[data-testid=prop-border-width]')?.focus();
    send(removeBorderAction(p));
  };
</script>

<Section title="Border">
  <div bind:this={widths} class="grid grid-cols-2 gap-2">
    <Field label="Border width" suffix="px" mono value={String(Math.round(p.border.width * 100) / 100)} onCommit={(raw) => send(textAction(p, { type: 'borderWidth' }, raw))} testId="prop-border-width" />
    <Field label="Border colour" mono value={p.border.color} onCommit={(raw) => send(textAction(p, { type: 'borderColor' }, raw))} testId="prop-border-color">
      {#snippet lead()}<i class="block size-4 shrink-0 rounded-sm border border-border" style:background={p.border.color}></i>{/snippet}
    </Field>
  </div>
  {#if on}
    <div class="flex items-center gap-2">
      <div class="min-w-0 flex-1">
        <Segmented label="Border position" options={BORDER_POSITIONS} value={p.border.position} onChange={(v) => send(borderPositionAction(p, v as typeof p.border.position))} data-testid="prop-border-position" />
      </div>
      <IconButton label="Remove border" icon="minus" size={24} onclick={remove} data-testid="prop-border-remove" />
    </div>
  {/if}
</Section>
