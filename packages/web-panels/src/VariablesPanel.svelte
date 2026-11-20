<!--
  The Variables section of the inspector, as the Variables, ModeSwitch and Inspector boards draw it: a search, the
  collections with their mode switch, and the variables listed by type with a swatch, the name and the value of the mode
  on view in the mono face (an alias as a chip with the name it points at). It draws the rows the app layer gives it
  (order, headings, checks) and reports input as actions (see variables.ts); it holds no state of its own.
-->
<script lang="ts">
  import type { Action, VariableRow, VariablesPanel } from '@meldframe/protocol';
  import Button from '@meldframe/web-chrome/Button.svelte';
  import Field from '@meldframe/web-chrome/Field.svelte';
  import Segmented from '@meldframe/web-chrome/Segmented.svelte';
  import { FIELD_LABEL, TYPE_LABEL, bindAction, display, listedCount, modeAction, shownValue, swatch, truncated, valueAction } from './variables';

  interface Props {
    panel: VariablesPanel;
    /** The id of the one selected node, which the bind buttons act on; none for no selection or several. */
    target: string | null;
    dispatch: (a: Action) => void;
  }
  let { panel, target, dispatch }: Props = $props();

  const commit = (row: VariableRow, mode: string) => (raw: string) => {
    const a = valueAction(row, mode, raw);
    if (a) dispatch(a);
  };
</script>

<div class="flex flex-col gap-2 text-small text-text" data-testid="variables">
  <Field label="Search variables" prefix="Search" size="dense" value={panel.query} onInput={(q) => dispatch({ type: 'setVariablesQuery', query: q })} testId="var-search" />
  <p class="m-0 text-right text-caption text-text-muted" data-testid="var-count">{panel.matched} of {panel.total}</p>
  {#each panel.collections as c (c.id)}
    <section class="flex flex-col gap-1" aria-label={c.name} data-testid="var-collection" data-id={c.id}>
      <div class="flex items-center justify-between gap-2">
        <h3 class="m-0 min-w-0 truncate text-small font-semibold text-text-strong" title={c.name}>{c.name}</h3>
        {#if c.modes.length > 1}
          <div class="min-w-0 max-w-40 flex-1">
            <Segmented label="Mode of {c.name}" options={c.modes.map((m) => ({ value: m.id, label: m.name }))} value={c.active} onChange={(m) => dispatch(modeAction(c, m))} data-testid="var-modes" />
          </div>
        {/if}
      </div>
      <ul class="m-0 flex list-none flex-col gap-px p-0">
        {#each c.variables as v (v.id)}
          {@const val = shownValue(c, v)}
          {@const colour = val ? swatch(v, val.mode) : null}
          {#if v.heading}
            <li class="px-1 pt-2 pb-1 text-text-muted" data-testid="var-heading">{TYPE_LABEL[v.heading.type]} <span class="text-caption">{v.heading.count}</span></li>
          {/if}
          <li class="flex flex-col gap-1 rounded-sm" data-testid="var-row" data-id={v.id}>
            <div class="flex items-center gap-2">
              <span class="min-w-0 flex-1 truncate text-text-secondary" title="{v.name} · {v.usedBy > 0 ? `used ${v.usedBy}` : 'unused'}">{v.name}</span>
              {#if val}
                <div class="w-36 shrink-0">
                  <Field label="{v.name} in {val.mode}" size="dense" mono bound={!!val.alias} value={display(val)} onCommit={commit(v, val.mode)} testId="var-value" data-var={v.id} data-mode={val.mode}>
                    {#snippet lead()}{#if colour}<i class="block size-4 shrink-0 rounded-sm border border-border" style:background={colour} data-testid="var-swatch"></i>{/if}{/snippet}
                  </Field>
                </div>
              {/if}
            </div>
            {#if target && v.binds.length}
              <div class="flex gap-1" role="group" aria-label="Bind {v.name} to the selection">
                {#each v.binds as b (b.field)}
                  <Button size="sm" pressed={b.bound} onclick={() => dispatch(bindAction(target, v, b.field, b.bound))} data-testid="var-bind" data-field={b.field}>{FIELD_LABEL[b.field]}</Button>
                {/each}
              </div>
            {/if}
          </li>
        {/each}
      </ul>
      {#if c.matched > c.variables.length}
        <p class="m-0 text-caption text-text-muted" data-testid="var-more">{c.matched - c.variables.length} more in {c.name}</p>
      {/if}
    </section>
  {/each}
  {#if panel.total === 0}
    <p class="m-0 text-text-muted">This file has no variables yet. An agent can import variables or add them.</p>
  {:else if panel.matched === 0}
    <p class="m-0 text-text-muted">No variable matches the search.</p>
  {/if}
  {#if truncated(panel)}
    <p class="m-0 text-caption text-text-muted" data-testid="var-truncated">Showing {listedCount(panel)} of {panel.matched}. Narrow the search to see the rest.</p>
  {/if}
</div>
