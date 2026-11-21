<!--
  One collection of the manage view: its name and what a removal of it touches, its modes (the first is the default one
  and stays), the form that adds a mode, the form that adds a variable, and the variables as a table with their type
  and how many times each is used. Every name is the file's text: drawn as text, direction characters left out. A
  removal does not happen here: it asks the parent, which shows the question. It holds only the type and the source of
  the values the next additions will take.
-->
<script lang="ts">
  import type { CollectionRow, VarType } from '@meldframe/protocol';
  import Button from '@meldframe/web-chrome/Button.svelte';
  import Segmented from '@meldframe/web-chrome/Segmented.svelte';
  import { ring } from '@meldframe/web-chrome/focus';
  import NameForm from './NameForm.svelte';
  import { count, label, type Ask, type Summary } from './manage';
  import { TYPE_LABEL } from './variables';

  interface Props {
    col: CollectionRow;
    summary: Summary;
    onAsk: (a: Ask) => void;
    onAddMode: (name: string, copyFrom: string) => void;
    onAddVariable: (name: string, kind: VarType) => void;
    /** Change when a mode or a variable was added: the form starts again. */
    modeKey: number;
    variableKey: number;
  }
  let { col, summary, onAsk, onAddMode, onAddVariable, modeKey, variableKey }: Props = $props();

  let kind = $state<VarType>('color');
  let from = $state('');
  // The mode the next one copies: the default (first) unless another was picked and is still there.
  const copy = $derived(col.modes.some((m) => m.id === from) ? from : (col.modes[0]?.id ?? ''));
  const name = $derived(label(col.name));
  const cut = $derived(col.matched > col.variables.length);
  const select = `${ring} h-(--mf-size-control-md) max-w-40 rounded-md border border-divider bg-field px-2 text-small text-text hov:border-border-strong`;
</script>

<div class="flex flex-col gap-4 p-4" data-testid="manage-detail-body" data-id={col.id}>
  <header class="flex items-start justify-between gap-3">
    <div class="min-w-0">
      <h3 tabindex="-1" class="m-0 truncate text-title font-semibold text-text-strong outline-none" title={name} data-testid="manage-title"><bdi>{name}</bdi></h3>
      <p class="m-0 text-caption text-text-muted" data-testid="manage-summary">{count(summary.variables, 'variable', 'variables')} · {summary.complete ? '' : 'at least '}{count(summary.uses, 'use', 'uses')} · {count(summary.modes, 'mode', 'modes')}</p>
    </div>
    <Button size="sm" variant="secondary" aria-label="Remove collection {name}" onclick={() => onAsk({ kind: 'collection', id: col.id })} data-testid="manage-remove-collection" data-remove="collection:{col.id}">Remove collection</Button>
  </header>

  <section class="flex flex-col gap-2" aria-label="Modes of {name}">
    <h4 class="m-0 text-body font-semibold text-text-strong">Modes</h4>
    <ul class="m-0 flex list-none flex-wrap gap-2 p-0">
      {#each col.modes as m, i (m.id)}
        <li tabindex="-1" class="{ring} flex items-center gap-2 rounded-md border border-divider bg-field py-1 pl-3 {i === 0 ? 'pr-3' : 'pr-1'} text-small text-text" data-testid="manage-mode" data-id={m.id} data-entity-id={m.id}>
          <span class="max-w-48 truncate" title={label(m.name)}><bdi>{label(m.name)}</bdi></span>
          {#if i === 0}
            <span class="text-caption text-text-muted" title="The first mode is the default one and stays">default</span>
          {:else}
            <Button size="sm" aria-label="Remove mode {label(m.name)}" onclick={() => onAsk({ kind: 'mode', collection: col.id, id: m.id })} data-testid="manage-remove-mode" data-remove="mode:{m.id}">Remove</Button>
          {/if}
        </li>
      {/each}
    </ul>
    {#key modeKey}
      <NameForm kind="mode" label="New mode name" submit="Add mode" placeholder="dark" existing={col.modes.map((m) => m.name)} onSubmit={(n) => onAddMode(n, copy)} testId="manage-new-mode">
        {#snippet extra()}
          <label class="flex items-center gap-2 text-small text-text-muted">
            <span aria-hidden="true">Copy from</span>
            <select aria-label="Copy the values of the new mode from" class={select} value={copy} onchange={(e) => (from = e.currentTarget.value)} data-testid="manage-copy-from">
              {#each col.modes as m (m.id)}<option value={m.id}>{label(m.name)}</option>{/each}
            </select>
          </label>
        {/snippet}
      </NameForm>
    {/key}
  </section>

  <section class="flex flex-col gap-2" aria-label="Variables of {name}">
    <h4 class="m-0 text-body font-semibold text-text-strong">Variables</h4>
    {#key variableKey}
      <NameForm kind="variable" label="New variable name" submit="Add variable" placeholder="color/brand-500" existing={col.variables.map((v) => v.name)} onSubmit={(n) => onAddVariable(n, kind)} testId="manage-new-variable">
        {#snippet extra()}
          <div class="w-40"><Segmented label="Type of the new variable" options={(['color', 'number'] as const).map((t) => ({ value: t, label: TYPE_LABEL[t] }))} value={kind} onChange={(v) => (kind = v as VarType)} data-testid="manage-new-type" /></div>
        {/snippet}
      </NameForm>
    {/key}
    {#if col.variables.length === 0}
      <p class="m-0 text-small text-text-muted" data-testid="manage-no-variables">{col.matched === 0 ? 'This collection has no variables yet. Add one above.' : 'No variable of this collection is listed.'}</p>
    {:else}
      <table class="w-full border-collapse text-small" data-testid="manage-vars">
        <caption class="sr-only">Variables of {name}, with their type and how many times each is used</caption>
        <thead>
          <tr class="border-b border-hairline text-left text-caption text-text-muted">
            <th scope="col" class="w-full py-1 pr-2 font-normal">Name</th>
            <th scope="col" class="px-2 py-1 font-normal">Type</th>
            <th scope="col" class="px-2 py-1 text-right font-normal">Uses</th>
            <th scope="col" class="py-1 pl-2 text-right font-normal"><span class="sr-only">Actions</span></th>
          </tr>
        </thead>
        <tbody>
          {#each col.variables as v (v.id)}
            <tr tabindex="-1" class="{ring} border-b border-hairline" data-testid="manage-var" data-id={v.id} data-entity-id={v.id}>
              <th scope="row" class="max-w-0 truncate py-1 pr-2 text-left font-normal text-text" title={label(v.name)}><bdi>{label(v.name)}</bdi></th>
              <td class="px-2 py-1 text-text-secondary" data-testid="manage-var-type">{TYPE_LABEL[v.type]}</td>
              <td class="px-2 py-1 text-right font-mono {v.usedBy === 0 ? 'text-text-muted' : 'text-text'}" data-testid="manage-var-uses" title={v.usedBy === 0 ? 'Nothing uses it' : count(v.usedBy, 'use', 'uses')}>{v.usedBy}</td>
              <td class="py-1 pl-2 text-right"><Button size="sm" aria-label="Remove {label(v.name)}" onclick={() => onAsk({ kind: 'variable', id: v.id })} data-testid="manage-remove-variable" data-remove="variable:{v.id}">Remove</Button></td>
            </tr>
          {/each}
        </tbody>
      </table>
      {#if cut}<p class="m-0 text-caption text-text-muted" data-testid="manage-cut">Showing {col.variables.length} of {col.matched} variables.</p>{/if}
    {/if}
  </section>
</div>
