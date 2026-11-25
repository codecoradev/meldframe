<!--
  The collections of the file, one button each (the chosen one is current), and the form that adds one. Names are the
  file's text and are drawn as text with direction characters left out. It holds no state of its own.
-->
<script lang="ts">
  import type { CollectionRow } from '@meldframe/protocol';
  import { ring } from '@meldframe/web-chrome/focus';
  import NameForm from './NameForm.svelte';
  import { count, nameOf } from './manage';

  interface Props {
    collections: CollectionRow[];
    /** The id of the collection the detail shows. */
    chosen: string | null;
    onChoose: (id: string) => void;
    onAdd: (name: string) => void;
    /** Changes when a collection was added: the form starts again. */
    resetKey: number;
  }
  let { collections, chosen, onChoose, onAdd, resetKey }: Props = $props();
</script>

<section class="flex flex-col gap-3 p-4" aria-label="Collections">
  <h3 tabindex="-1" class="m-0 text-body font-semibold text-text-strong outline-none" data-testid="manage-collections-title">Collections</h3>
  {#if collections.length === 0}
    <p class="m-0 text-small text-text-muted" data-testid="manage-no-collections">This file has no collections yet. Add one to start.</p>
  {/if}
  <ul class="m-0 flex list-none flex-col gap-px p-0">
    {#each collections as c (c.id)}
      {@const on = c.id === chosen}
      <li>
        <button
          type="button"
          aria-current={on ? 'true' : undefined}
          onclick={() => onChoose(c.id)}
          data-testid="manage-collection"
          data-id={c.id}
          data-entity-id={c.id}
          class="{ring} flex w-full flex-col rounded-md px-2 py-1 text-left hov:bg-control {on ? 'bg-chip-bg text-chip-text' : 'text-text'}"
        >
          <span class="truncate text-small font-medium" title={nameOf(c.name)}><bdi>{nameOf(c.name)}</bdi></span>
          <span class="text-caption {on ? '' : 'text-text-muted'}">{count(c.matched, 'variable', 'variables')} · {count(c.modes.length, 'mode', 'modes')}</span>
        </button>
      </li>
    {/each}
  </ul>
  {#key resetKey}
    <NameForm kind="collection" label="New collection name" submit="Add collection" placeholder="color" existing={collections.map((c) => c.name)} onSubmit={onAdd} testId="manage-new-collection" />
  {/key}
</section>
