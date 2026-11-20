<!--
  How the width and the height of the node are sized (fixed, hug or fill), under the Size fields, and the notes that say
  a choice has no effect where the node is. The notes are the app layer's; this words them. It draws the document and
  reports input as actions (see layout.ts); it holds no state of its own.
-->
<script lang="ts">
  import type { Action, Properties } from '@meldframe/protocol';
  import Segmented from '@meldframe/web-chrome/Segmented.svelte';
  import { noteText, SIZE_MODES, sizingAction } from '@meldframe/web-document/layout';

  interface Props {
    properties: Properties;
    dispatch: (a: Action) => void;
  }
  let { properties: p, dispatch }: Props = $props();
  const send = (a: Action | null) => {
    if (a) dispatch(a);
  };
</script>

<div class="flex flex-col gap-2">
  <span class="text-text-muted">Width sizing</span>
  <Segmented label="Width sizing" options={SIZE_MODES} value={p.sw} onChange={(v) => send(sizingAction(p, 'width', v as typeof p.sw))} data-testid="prop-sw" />
  <span class="text-text-muted">Height sizing</span>
  <Segmented label="Height sizing" options={SIZE_MODES} value={p.sh} onChange={(v) => send(sizingAction(p, 'height', v as typeof p.sh))} data-testid="prop-sh" />
  {#each p.notes as note, i (i)}
    <p class="m-0 text-caption text-text-muted" role="note" data-testid="prop-note">{noteText(note)}</p>
  {/each}
</div>
