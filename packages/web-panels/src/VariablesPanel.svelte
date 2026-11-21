<!--
  The Variables section of the inspector, as the Variables, ModeSwitch and Inspector boards draw it: a search, the
  collections with their mode switch, and the variables listed by type with a swatch, the name and the value of the mode
  on view in the mono face (an alias as a chip with the name it points at). It draws the rows the app layer gives it
  (order, headings, checks) and reports input as actions (see variables.ts). It holds one bit of view state: whether the
  manage view (ManageDialog) is open. The Export variables dialog is open while the app layer holds its text.
-->
<script lang="ts">
  import type { Action, Event, VariableRow, VariablesIo, VariablesPanel } from '@meldframe/protocol';
  import Button from '@meldframe/web-chrome/Button.svelte';
  import Field from '@meldframe/web-chrome/Field.svelte';
  import Segmented from '@meldframe/web-chrome/Segmented.svelte';
  import { TYPE_LABEL } from '@meldframe/web-document/labels';
  import ExportVariablesDialog from '@meldframe/web-variables-io/ExportVariablesDialog.svelte';
  import ManageDialog from '@meldframe/web-variables/ManageDialog.svelte';
  import { FIELD_LABEL, bindAction, display, listedCount, modeAction, modeName, shownValue, swatch, truncated, valueAction } from './variables';

  interface Props {
    panel: VariablesPanel;
    /** The id of the one selected node, which the bind buttons act on; none for no selection or several. */
    target: string | null;
    dispatch: (a: Action) => void;
    /** The events of the last dispatch: the manage view words a refusal and finds what it added from them. */
    events?: readonly Event[];
    /** `state.variablesIo`: the text of the Export variables dialog while it is open, and the result of the last import. */
    io?: VariablesIo;
  }
  let { panel, target, dispatch, events = [], io = { export: null, import: null } }: Props = $props();

  // The manage view reads the whole list: it starts with the search cleared (the counts of a removal are of what is listed).
  let manageOpen = $state(false);
  let manageButton = $state<HTMLElement | null>(null);
  const setManage = (open: boolean) => {
    manageOpen = open;
    if (open && panel.query.trim() !== '') dispatch({ type: 'setVariablesQuery', query: '' });
  };

  // Export variables is open while the app layer holds its text.
  let exportButton = $state<HTMLElement | null>(null);

  const commit = (row: VariableRow, mode: string) => (raw: string) => {
    const a = valueAction(row, mode, raw);
    if (a) dispatch(a);
  };
</script>

<div class="flex flex-col gap-2 text-small text-text" data-testid="variables">
  <Field label="Search variables" prefix="Search" size="dense" value={panel.query} onInput={(q) => dispatch({ type: 'setVariablesQuery', query: q })} testId="var-search" />
  <div class="flex items-center justify-between gap-2">
    <span bind:this={manageButton} class="contents"><Button size="sm" onclick={() => setManage(true)} data-testid="var-manage">Manage</Button></span>
    <p class="m-0 text-right text-caption text-text-muted" data-testid="var-count">{panel.matched} of {panel.total}</p>
  </div>
  <div class="flex flex-wrap items-center gap-2">
    <span bind:this={exportButton} class="contents"><Button size="sm" onclick={() => dispatch({ type: 'openVariablesExport' })} data-testid="var-export">Export variables</Button></span>
  </div>
  <ExportVariablesDialog dialog={io.export ?? null} {dispatch} opener={() => exportButton?.querySelector('button')} />
  <ManageDialog open={manageOpen} onOpenChange={setManage} {panel} {events} {dispatch} opener={() => manageButton?.querySelector('button')} />
  {#each panel.collections.filter((c) => c.modes.length > 1) as c (c.id)}
    <div class="flex flex-col gap-1">
      <span class="truncate text-caption font-semibold text-text-muted" title={c.name}>{c.name}</span>
      <Segmented label="Mode of {c.name}" options={c.modes.map((m) => ({ value: m.id, label: m.name }))} value={c.active} onChange={(m) => dispatch(modeAction(c, m))} data-testid="var-modes" data-id={c.id} />
    </div>
  {/each}
  {#if panel.collections.some((c) => c.modes.length > 1)}
    <p class="m-0 text-caption text-text-muted">Switch mode to edit its values.</p>
  {/if}
  <div class="max-h-(--mf-size-variables-list) overflow-y-auto" data-testid="var-list">
    {#each panel.collections as c (c.id)}
      <section class="flex flex-col gap-1" aria-label={c.name} data-testid="var-collection" data-id={c.id}>
        {#if panel.collections.length > 1}
          <h3 class="m-0 truncate pt-2 text-small font-semibold text-text-strong" title={c.name}>{c.name}</h3>
        {/if}
        <ul class="m-0 flex list-none flex-col gap-px p-0">
          {#each c.variables as v (v.id)}
            {@const val = shownValue(c, v)}
            {@const colour = val ? swatch(v, val.mode) : null}
            {@const boundNow = v.binds.some((b) => b.bound)}
            {#if v.heading}
              <li class="px-1 pt-2 pb-1"><h4 class="m-0 text-small font-normal text-text-muted" data-testid="var-heading">{TYPE_LABEL[v.heading.type]} <span class="text-caption">{v.heading.count}</span></h4></li>
            {/if}
            <li class="group flex items-center gap-2" data-testid="var-row" data-id={v.id}>
              <div class="relative min-w-0 flex-1">
                <span class="block truncate {boundNow ? 'font-medium text-chip-text' : 'text-text-secondary'} {target && v.binds.length ? 'group-hover:invisible group-focus-within:invisible' : ''}" title="{v.name} · {v.usedBy > 0 ? `used ${v.usedBy}` : 'unused'}">{v.name}</span>
                {#if target && v.binds.length}
                  <div class="absolute inset-y-0 left-0 flex items-center gap-1 opacity-0 group-hover:opacity-100 group-focus-within:opacity-100" role="group" aria-label="Bind {v.name} to the selection">
                    {#each v.binds as b (b.field)}
                      <Button size="sm" pressed={b.bound} onclick={() => dispatch(bindAction(target, v, b.field, b.bound))} data-testid="var-bind" data-field={b.field}>{FIELD_LABEL[b.field]}</Button>
                    {/each}
                  </div>
                {/if}
              </div>
              {#if val}
                <div class="w-28 shrink-0" title={display(val)}>
                  <Field label="{v.name} in {modeName(c, val.mode)}" size="dense" mono bound={!!val.alias} value={display(val)} onCommit={commit(v, val.mode)} testId="var-value">
                    {#snippet lead()}{#if colour}<i class="block size-4 shrink-0 rounded-sm border border-border" style:background={colour} data-testid="var-swatch"></i>{/if}{/snippet}
                  </Field>
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
  </div>
  {#if panel.total === 0}
    <p class="m-0 text-text-muted">This file has no variables yet. An agent can import variables or add them.</p>
  {:else if panel.matched === 0}
    <p class="m-0 text-text-muted">No variable matches the search.</p>
  {/if}
  {#if truncated(panel)}
    <p class="m-0 text-caption text-text-muted" data-testid="var-truncated">Showing {listedCount(panel)} of {panel.matched}. Narrow the search to see the rest.</p>
  {/if}
</div>
