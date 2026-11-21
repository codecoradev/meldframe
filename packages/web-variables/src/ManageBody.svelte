<!--
  The manage view of the Variables (D8, the VariableManagement board): the collections on the left, the chosen one on
  the right with its modes and variables, and the question before a removal. It reads the panel the app layer gives it
  (`AppState.variables`) and sends the six manage actions; the ids are the app layer's, so the thing that was added is
  found by `Event::EntityAdded` and given the focus. What it holds is view state only: which collection is shown, the
  removal being asked about, the text of the last refusal or change (the live regions) and a counter for each form,
  bumped to start the form again. The name rules are `manage.ts`; the words of a refusal are `notices.ts`.
-->
<script lang="ts">
  import { tick, untrack } from 'svelte';
  import type { Action, Event, VarType, VariablesPanel } from '@meldframe/protocol';
  import Button from '@meldframe/web-chrome/Button.svelte';
  import CollectionDetail from './CollectionDetail.svelte';
  import CollectionList from './CollectionList.svelte';
  import RemoveConfirm from './RemoveConfirm.svelte';
  import { addModeAction, addVariableAction, collectionSummary, createCollectionAction, label, partial, reactTo, removalPlan, targetOf, whereAdded, type Ask } from './manage';

  interface Props {
    panel: VariablesPanel;
    /** The events of the last dispatch (`snapshot.events`). */
    events: readonly Event[];
    dispatch: (a: Action) => void;
  }
  let { panel, events, dispatch }: Props = $props();

  let root = $state<HTMLElement | null>(null);
  let chosenId = $state<string | null>(null);
  const current = $derived(panel.collections.find((c) => c.id === chosenId) ?? panel.collections[0] ?? null);
  const summary = $derived(current ? collectionSummary(current, panel) : null);
  const why = $derived(partial(panel));

  let ask = $state<Ask | null>(null);
  const target = $derived(ask ? targetOf(panel, ask) : null);
  const plan = $derived(target ? removalPlan(target) : null);
  // The question belongs to a thing that is still there: another actor may have removed it meanwhile.
  $effect(() => {
    if (ask && !target) ask = null;
  });

  // The live regions are always on the page and the text is put into them (a region that appears with its text is not announced).
  let error = $state('');
  let status = $state('');
  let keys = $state({ collection: 0, mode: 0, variable: 0 });
  // The words of the change just sent, said when the document answers; and the question to give the focus back after it.
  let pending: string | null = null;
  // What was sent last: the words of a limit depend on it (`reactTo`).
  let sent: Action | null = null;
  let refocus: Ask | null = null;

  const q = (match: (el: HTMLElement) => boolean) => [...(root?.querySelectorAll<HTMLElement>('[data-entity-id], [data-remove], [data-testid=manage-title], [data-testid=manage-collection], [data-testid=manage-collections-title]') ?? [])].find(match);
  const focusEntity = (id: string) => {
    const el = q((e) => e.dataset.entityId === id);
    el?.scrollIntoView({ block: 'nearest' });
    el?.focus();
  };
  /** Back to the button that asked; when that thing is gone, to the title of the collection, then to a collection, then to the heading of the list. */
  const focusAsker = (a: Ask) => {
    const key = `${a.kind}:${a.id}`;
    (q((e) => e.dataset.remove === key) ?? q((e) => e.dataset.testid === 'manage-title') ?? q((e) => e.dataset.testid === 'manage-collection') ?? q((e) => e.dataset.testid === 'manage-collections-title'))?.focus();
  };

  const send = (a: Action, done: string) => {
    pending = done;
    sent = a;
    dispatch(a);
  };

  let seen = untrack(() => events);
  $effect(() => {
    const now = events;
    if (now === seen) return;
    seen = now;
    const r = reactTo(now, untrack(() => pending), untrack(() => sent));
    if (r.error) {
      error = r.error;
      status = '';
      pending = null;
    } else if (r.status || r.added) {
      error = '';
      if (r.status) status = r.status;
      pending = null;
    }
    const added = r.added;
    if (added) {
      const where = whereAdded(panel, added);
      if (where) chosenId = where;
      keys = { ...keys, [added.entity]: keys[added.entity] + 1 };
      void tick().then(() => focusEntity(added.id));
    }
    if (refocus && (r.error || r.status)) {
      const a = refocus;
      refocus = null;
      void tick().then(() => focusAsker(a));
    }
  });

  const close = () => {
    const a = ask;
    ask = null;
    if (a) void tick().then(() => focusAsker(a));
  };
  const confirm = () => {
    const p = plan;
    refocus = ask;
    ask = null;
    if (p) send(p.action, p.done);
  };
</script>

<div bind:this={root} class="flex min-h-0 flex-1 flex-col" data-testid="manage">
  {#if why}
    <p class="m-0 flex items-center justify-between gap-3 border-b border-hairline bg-warning-bg px-4 py-2 text-small text-text-strong" data-testid="manage-partial">
      {#if why === 'search'}
        <span>The search of the Variables panel narrows this list, so the counts cover only what it matches.</span>
        <Button size="sm" onclick={() => dispatch({ type: 'setVariablesQuery', query: '' })} data-testid="manage-clear-search">Clear search</Button>
      {:else}
        <span>This file has more variables than the list holds, so some counts are partial.</span>
      {/if}
    </p>
  {/if}
  <div class="flex min-h-0 flex-1">
    <div class="w-64 shrink-0 overflow-y-auto border-r border-hairline">
      <CollectionList collections={panel.collections} chosen={current?.id ?? null} onChoose={(id) => { chosenId = id; ask = null; }} onAdd={(n) => send(createCollectionAction(n), `Added the collection “${label(n)}”.`)} resetKey={keys.collection} />
    </div>
    <div class="min-w-0 flex-1 overflow-y-auto" data-testid="manage-detail">
      {#if plan && ask}
        <!-- A question for another thing starts again, so it takes the focus on Cancel as the first one did. -->
        {#key `${ask.kind}:${ask.id}`}
          <div class="px-4 pt-4"><RemoveConfirm {plan} onConfirm={confirm} onCancel={close} /></div>
        {/key}
      {/if}
      {#if current && summary}
        <CollectionDetail
          col={current}
          {summary}
          onAsk={(a) => (ask = a)}
          onAddMode={(n, from) => send(addModeAction(current.id, n, from), `Added the mode “${label(n)}”.`)}
          onAddVariable={(n, kind: VarType) => send(addVariableAction(current.id, n, kind), `Added the variable “${label(n)}”.`)}
          modeKey={keys.mode}
          variableKey={keys.variable}
        />
      {/if}
    </div>
  </div>
  <div class="border-t border-hairline px-4 py-2">
    <div role="alert" data-testid="manage-error">
      {#if error}<p class="m-0 rounded-md bg-danger-bg px-3 py-2 text-small text-danger-text">{error}</p>{/if}
    </div>
    <div role="status" data-testid="manage-status">
      {#if status && !error}<p class="m-0 px-1 text-small text-text-secondary">{status}</p>{/if}
    </div>
  </div>
</div>
