<!--
  What is inside the bind picker of a field: the search, the variables that fit the field (a bound one is pressed),
  and the notes under them. The app layer says which type fits (`picker.type`) and lists the rows; this draws them.
  It holds no state of its own.
-->
<script lang="ts">
  import type { Action, Picker } from '@meldframe/protocol';
  import Field from '@meldframe/web-chrome/Field.svelte';
  import { TYPE_LABEL } from '@meldframe/web-document/labels';
  import { nameOf } from '@meldframe/web-document/names';
  import { pickAction, pickerQueryAction } from '@meldframe/web-document/picker';
  import { FIELD_LABEL } from './variables';

  interface Props {
    picker: Picker;
    dispatch: (a: Action) => void;
  }
  let { picker, dispatch }: Props = $props();
  const type = $derived(TYPE_LABEL[picker.type].toLowerCase());
  const label = $derived(FIELD_LABEL[picker.field]);
</script>

<div class="border-b border-menu-border p-2">
  <Field label="Search variables" prefix="Search" size="dense" value={picker.query} onInput={(q) => dispatch(pickerQueryAction(q))} testId="picker-search" />
</div>
<ul class="m-0 flex max-h-[calc(var(--mf-size-control-lg)*6)] list-none flex-col gap-px overflow-y-auto p-2" aria-label="Variables for {label}">
  {#each picker.rows as row (row.id)}
    <li>
      <button type="button" aria-pressed={row.bound} onclick={() => dispatch(pickAction(picker, row))} data-testid="picker-row" data-id={row.id} class="flex h-(--mf-size-control-lg) w-full items-center gap-2 rounded-sm px-2 text-left hov:bg-control {row.bound ? 'bg-chip-bg text-chip-text' : ''}">
        {#if row.type === 'color' && row.resolved}<i class="block size-4 shrink-0 rounded-sm border border-border" style:background={row.resolved}></i>{/if}
        <span class="min-w-0 flex-1 truncate" title="{nameOf(row.name)} · {nameOf(row.collection)}">{nameOf(row.name)}</span>
        <span class="font-mono text-caption text-text-muted">{row.resolved}</span>
      </button>
    </li>
  {/each}
</ul>
{#if picker.rows.length === 0}
  <p class="m-0 px-4 pb-2 text-text-muted" data-testid="picker-empty">{picker.total === 0 ? `This file has no ${type} variables.` : 'No variable matches the search.'}</p>
{/if}
<p class="m-0 border-t border-menu-border px-4 py-2 text-caption text-text-muted">Only {type} variables are shown for {label}.</p>
