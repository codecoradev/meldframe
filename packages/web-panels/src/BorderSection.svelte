<!--
  The Border section as the Properties and Fill-and-layout boards draw it: the width, the colour with its swatch, and
  where the border sits against the edge. A width of 0 is no border. It draws the document and reports input as actions
  (see decoration.ts); it holds no state of its own.
-->
<script lang="ts">
  import type { Action, Properties } from '@meldframe/protocol';
  import Field from '@meldframe/web-chrome/Field.svelte';
  import Section from '@meldframe/web-chrome/Section.svelte';
  import Segmented from '@meldframe/web-chrome/Segmented.svelte';
  import { BORDER_POSITIONS, borderColorAction, borderPositionAction, borderWidthAction } from '@meldframe/web-document/decoration';

  interface Props {
    properties: Properties;
    dispatch: (a: Action) => void;
  }
  let { properties: p, dispatch }: Props = $props();
  const send = (a: Action | null) => {
    if (a) dispatch(a);
  };
  const on = $derived(p.border.width > 0);
</script>

<Section title="Border">
  <div class="grid grid-cols-2 gap-2">
    <Field label="Border width" suffix="px" mono value={String(Math.round(p.border.width * 100) / 100)} onCommit={(raw) => send(borderWidthAction(p, raw))} testId="prop-border-width" />
    <Field label="Border colour" mono value={p.border.color} onCommit={(raw) => send(borderColorAction(p, raw))} testId="prop-border-color">
      {#snippet lead()}<i class="block size-4 shrink-0 rounded-sm border border-border" style:background={p.border.color}></i>{/snippet}
    </Field>
  </div>
  {#if on}
    <Segmented label="Border position" options={BORDER_POSITIONS} value={p.border.position} onChange={(v) => send(borderPositionAction(p, v as typeof p.border.position))} data-testid="prop-border-position" />
  {/if}
</Section>
