<!--
  The Variables panel: the collections with their modes, the variables with a value for each mode, a search, and
  the buttons that bind the selected node to a variable. It draws the rows the app layer gives it and reports input as
  actions (see variables.ts); it holds no state of its own.
-->
<script lang="ts">
  import { tick } from 'svelte';
  import type { Action, ModeValue, VariableRow, VariablesPanel } from '@meldframe/protocol';
  import { bindAction, listedCount, modeAction, swatch, truncated, valueAction } from './variables';

  interface Props {
    panel: VariablesPanel;
    /** The id of the one selected node, which the bind buttons act on; none for no selection or several. */
    target: string | null;
    dispatch: (a: Action) => void;
  }
  let { panel, target, dispatch }: Props = $props();

  const input = 'min-w-0 flex-1 rounded-sm border border-border bg-field px-2 py-1 font-mono text-text outline-none focus:border-accent';
  const FIELDS = { fill: 'Fill', radius: 'Radius', opacity: 'Opacity' } as const;

  /** A value as its field shows it: an alias as the name of its target. */
  const display = (val: ModeValue): string => (val.alias ? `→ ${val.text}` : val.text);
  /** What the app layer holds for a value now. */
  const shown = (id: string, mode: string): string => {
    const val = panel.collections.flatMap((c) => c.variables).find((v) => v.id === id)?.values.find((v) => v.mode === mode);
    return val ? display(val) : '';
  };

  function keys(e: KeyboardEvent) {
    const el = e.currentTarget as HTMLInputElement;
    if (e.key === 'Enter') el.blur();
    else if (e.key === 'Escape') {
      el.value = el.defaultValue;
      el.blur();
    }
    e.stopPropagation(); // typing here is text, not a command of the editor
  }

  function commit(el: HTMLInputElement, row: VariableRow, mode: string) {
    const a = valueAction(row, mode, el.value);
    if (a) dispatch(a);
    // After the app took the change (or refused it) the field shows what it holds.
    void tick().then(() => (el.value = shown(row.id, mode)));
  }
</script>

<aside class="flex w-72 shrink-0 flex-col border-l border-hairline bg-sidebar text-small text-text" aria-label="Variables" data-testid="variables">
  <header class="flex items-baseline justify-between border-b border-hairline px-3 py-2 text-text-muted">
    <span>Variables</span>
    <span class="text-caption" data-testid="var-count">{panel.matched} of {panel.total}</span>
  </header>
  <div class="border-b border-hairline p-2">
    <input
      type="search"
      class="{input} w-full"
      placeholder="Search variables"
      aria-label="Search variables"
      value={panel.query}
      data-testid="var-search"
      onkeydown={(e) => e.stopPropagation()}
      oninput={(e) => dispatch({ type: 'setVariablesQuery', query: e.currentTarget.value })}
    />
  </div>
  <div class="min-h-0 flex-1 overflow-y-auto">
    {#each panel.collections as c (c.id)}
      <section class="border-b border-hairline" aria-label={c.name} data-testid="var-collection" data-id={c.id}>
        <div class="flex items-center justify-between gap-2 px-3 py-2">
          <h3 class="truncate text-text-strong">{c.name}</h3>
          <div class="flex shrink-0 gap-1" role="group" aria-label="Mode of {c.name}">
            {#each c.modes as m (m.id)}
              <button
                type="button"
                class="rounded-md px-2 py-1 text-caption {c.active === m.id ? 'bg-accent text-on-accent' : 'bg-field text-text-muted hover:text-text'}"
                aria-pressed={c.active === m.id}
                data-testid="var-mode"
                data-mode={m.id}
                onclick={() => dispatch(modeAction(c, m.id))}
              >{m.name}</button>
            {/each}
          </div>
        </div>
        <ul>
          {#each c.variables as v (v.id)}
            <li class="flex flex-col gap-1 px-3 py-2 hover:bg-field" data-testid="var-row" data-id={v.id}>
              <div class="flex items-baseline justify-between gap-2">
                <span class="truncate text-text-strong" title={v.name}>{v.name}</span>
                <span class="shrink-0 text-caption text-text-faint">{v.usedBy > 0 ? `used ${v.usedBy}` : 'unused'}</span>
              </div>
              {#each v.values as val (val.mode)}
                {@const colour = swatch(v, val.mode)}
                <label class="flex items-center gap-2">
                  <span class="w-10 shrink-0 truncate text-caption text-text-muted" title={val.mode}>{val.mode}</span>
                  {#if colour}
                    <i class="block h-4 w-4 shrink-0 rounded-sm border border-border" style:background={colour} data-testid="var-swatch"></i>
                  {/if}
                  <input
                    class={input}
                    value={display(val)}
                    aria-label="{v.name} in {val.mode}"
                    data-testid="var-value"
                    data-var={v.id}
                    data-mode={val.mode}
                    onkeydown={keys}
                    onchange={(e) => commit(e.currentTarget, v, val.mode)}
                  />
                </label>
              {/each}
              {#if target && v.binds.length}
                <div class="flex gap-1" role="group" aria-label="Bind {v.name} to the selection">
                  {#each v.binds as b (b.field)}
                    <button
                      type="button"
                      class="rounded-md px-2 py-1 text-caption {b.bound ? 'bg-accent text-on-accent' : 'bg-field text-text-muted hover:text-text'}"
                      aria-pressed={b.bound}
                      title={b.bound ? `Unbind ${FIELDS[b.field]}` : `Bind ${FIELDS[b.field]} to ${v.name}`}
                      data-testid="var-bind"
                      data-field={b.field}
                      onclick={() => dispatch(bindAction(target, v, b.field, b.bound))}
                    >{FIELDS[b.field]}</button>
                  {/each}
                </div>
              {/if}
            </li>
          {/each}
        </ul>
      </section>
    {/each}
    {#if panel.total === 0}
      <p class="px-3 py-3 text-text-faint">This file has no variables yet. An agent can import a theme or add them.</p>
    {:else if panel.matched === 0}
      <p class="px-3 py-3 text-text-faint">No variable matches the search.</p>
    {/if}
    {#if truncated(panel)}
      <p class="px-3 py-2 text-caption text-text-faint" data-testid="var-truncated">Showing {listedCount(panel)} of {panel.matched}. Narrow the search to see the rest.</p>
    {/if}
  </div>
</aside>
