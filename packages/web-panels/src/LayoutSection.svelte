<!--
  The Layout section of a frame as the Properties and Fill-and-layout boards draw it: free, row or column; for a row or a
  column the gap, where the children sit across and along it, and the padding (one value for all sides, and each side).
  It draws the document and reports input as actions (see layout.ts); it holds no state of its own.
-->
<script lang="ts">
  import type { Action, Properties } from '@meldframe/protocol';
  import Field from '@meldframe/web-chrome/Field.svelte';
  import Section from '@meldframe/web-chrome/Section.svelte';
  import Segmented from '@meldframe/web-chrome/Segmented.svelte';
  import { ALIGNS, alignAction, gapAction, JUSTIFIES, justifyAction, LAYOUT_MODES, layoutAction, paddingAllAction, paddingSideAction, SIDES } from '@meldframe/web-document/layout';

  interface Props {
    properties: Properties;
    dispatch: (a: Action) => void;
  }
  let { properties: p, dispatch }: Props = $props();
  const send = (a: Action | null) => {
    if (a) dispatch(a);
  };
  // The app layer says the value the four sides share; when they differ the field is empty and says Mixed.
  const all = $derived(p.paddingUniform);
</script>

<Section title="Layout">
  <Segmented label="Layout direction" options={LAYOUT_MODES} value={p.layout} onChange={(v) => send(layoutAction(p, v as typeof p.layout))} data-testid="prop-layout" />
  {#if p.layout !== 'none'}
    <div class="grid grid-cols-2 gap-2">
      <Field label="Gap" prefix="Gap" suffix="px" mono value={String(p.gap)} onCommit={(raw) => send(gapAction(p, raw))} testId="prop-gap" />
      <Field label="Padding, all sides" prefix="Pad" suffix="px" mono value={all === undefined ? '' : String(all)} placeholder={all === undefined ? 'Mixed' : undefined} onCommit={(raw) => send(paddingAllAction(p, raw))} testId="prop-padding" />
    </div>
    <div class="grid grid-cols-2 gap-2">
      {#each SIDES as s (s.side)}
        <Field label="Padding {s.label.toLowerCase()}" prefix={s.label[0]} mono value={String(p.padding[s.side])} onCommit={(raw) => send(paddingSideAction(p, s.side, raw))} testId="prop-padding-{s.side}" />
      {/each}
    </div>
    <span class="text-text-muted">Align</span>
    <Segmented label="Align children" options={ALIGNS} value={p.align} onChange={(v) => send(alignAction(p, v as typeof p.align))} data-testid="prop-align" />
    <span class="text-text-muted">Justify</span>
    <Segmented label="Justify children" options={JUSTIFIES} value={p.justify} onChange={(v) => send(justifyAction(p, v as typeof p.justify))} data-testid="prop-justify" />
  {/if}
</Section>
