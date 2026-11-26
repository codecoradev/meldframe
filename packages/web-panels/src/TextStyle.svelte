<!--
  The weight and the alignment of a text, in the Text section under the font size: a choice of the nine weights the model
  takes and of left, center or right, each under a caption as the fields above are. It draws the document and reports
  input as actions (see properties.ts); it holds no state of its own. The weight is a native select, so the keyboard and
  the screen reader get it as they know it.
-->
<script lang="ts">
  import { tick } from 'svelte';
  import type { Action, Properties, TextAlign } from '@meldframe/protocol';
  import { ring } from '@meldframe/web-chrome/focus';
  import Segmented from '@meldframe/web-chrome/Segmented.svelte';
  import { FONT_WEIGHTS, fontWeightAction, TEXT_ALIGNS, textAlignAction } from '@meldframe/web-document/properties';
  import { alignOf, weightOf } from './fields';

  interface Props {
    properties: Properties;
    dispatch: (a: Action) => void;
  }
  let { properties: p, dispatch }: Props = $props();
  const send = (a: Action | null) => {
    if (a) dispatch(a);
  };
  const weight = $derived(weightOf(p));
  const select = `${ring} h-(--mf-size-control-lg) w-full rounded-md border border-divider bg-field px-2 text-small text-text hov:border-border-strong`;
</script>

<div class="flex flex-col gap-2">
  <label for="prop-fontWeight" class="text-text-muted">Font weight</label>
  <select
    id="prop-fontWeight"
    class={select}
    value={weight}
    onchange={(e) => {
      const el = e.currentTarget;
      send(fontWeightAction(p, el.value));
      // A weight the document did not take is undone: the select shows the document's value again.
      void tick().then(() => (el.value = weight));
    }}
    data-testid="prop-fontWeight"
  >
    {#each FONT_WEIGHTS as w (w.value)}<option value={w.value}>{w.label}</option>{/each}
  </select>
</div>
<div class="flex flex-col gap-2">
  <span class="text-text-muted">Text alignment</span>
  <Segmented label="Text alignment" iconOnly options={TEXT_ALIGNS} value={alignOf(p)} onChange={(v) => send(textAlignAction(p, v as TextAlign))} data-testid="prop-textAlign" />
</div>
