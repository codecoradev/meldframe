<!--
  The Shadow section as the Properties board draws it: a row for each drop shadow (offset, blur, spread and colour, in
  the order they are drawn) with a button to remove it, and a button to add one, up to the limit the app layer gives. It draws the document and
  reports input as actions (see decoration.ts); it holds no state of its own.
-->
<script lang="ts">
  import type { Action, Properties } from '@meldframe/protocol';
  import Button from '@meldframe/web-chrome/Button.svelte';
  import Field from '@meldframe/web-chrome/Field.svelte';
  import IconButton from '@meldframe/web-chrome/IconButton.svelte';
  import Section from '@meldframe/web-chrome/Section.svelte';
  import { addShadowAction, removeShadowAction, SHADOW_NUMBERS, shadowAction } from '@meldframe/web-document/decoration';

  interface Props {
    properties: Properties;
    dispatch: (a: Action) => void;
  }
  let { properties: p, dispatch }: Props = $props();
  const send = (a: Action | null) => {
    if (a) dispatch(a);
  };
  const round = (v: number) => String(Math.round(v * 100) / 100);
</script>

<Section title="Shadow">
  {#each p.shadows as s, i (i)}
    <div class="flex flex-col gap-2" data-testid="prop-shadow">
      <div class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2">
        <Field label="Shadow {i + 1} colour" mono value={s.color} onCommit={(raw) => send(shadowAction(p, i, 'color', raw))} testId="prop-shadow-color">
          {#snippet lead()}<i class="block size-4 shrink-0 rounded-sm border border-border" style:background={s.color}></i>{/snippet}
        </Field>
        <IconButton label="Remove shadow {i + 1}" icon="minus" size={24} onclick={() => send(removeShadowAction(p, i))} data-testid="prop-shadow-remove" />
      </div>
      <div class="grid grid-cols-2 gap-2">
        {#each SHADOW_NUMBERS as n (n.field)}
          <Field label="Shadow {i + 1} {n.label.toLowerCase()}" prefix={n.label} mono value={round(s[n.field])} onCommit={(raw) => send(shadowAction(p, i, n.field, raw))} testId="prop-shadow-{n.field}" />
        {/each}
      </div>
    </div>
  {/each}
  <div class="flex"><Button size="sm" disabled={p.shadows.length >= p.maxShadows} onclick={() => send(addShadowAction(p))} data-testid="prop-shadow-add">Add shadow</Button></div>
</Section>
