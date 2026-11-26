<!--
  The placement of a node inside an auto layout parent as the Fill-and-layout board draws it: its margin on each side,
  and whether it leaves the flow (absolute). Shown in an auto layout parent, and elsewhere only while the node has a margin or is absolute (the notes say they do
  nothing there, and these fields clear them).
  It draws the document and reports input as actions (see layout.ts); it holds no state of its own.
-->
<script lang="ts">
  import type { Action, Properties } from '@meldframe/protocol';
  import Checkbox from '@meldframe/web-chrome/Checkbox.svelte';
  import Field from '@meldframe/web-chrome/Field.svelte';
  import Section from '@meldframe/web-chrome/Section.svelte';
  import { absoluteAction, SIDES } from '@meldframe/web-document/layout';
  import { textAction } from '@meldframe/web-document/properties';

  interface Props {
    properties: Properties;
    dispatch: (a: Action) => void;
  }
  let { properties: p, dispatch }: Props = $props();
  const send = (a: Action | null) => {
    if (a) dispatch(a);
  };
</script>

<Section title="Margin">
  <div class="grid grid-cols-2 gap-2">
    {#each SIDES as s (s.side)}
      <Field label="Margin {s.label.toLowerCase()}" prefix={s.label[0]} mono value={String(p.margin[s.side])} onCommit={(raw) => send(textAction(p, { type: 'margin', side: s.side }, raw))} testId="prop-margin-{s.side}" />
    {/each}
  </div>
  <Checkbox label="Absolute position" checked={p.absolute} onChange={(c) => send(absoluteAction(p, c))} data-testid="prop-absolute" />
</Section>
