<!--
  The manage view of the Variables (D8, the VariableManagement board): the collections on the left, the chosen one on
  the right with its modes and variables, and the question before a removal. It reads the panel and the manage state the app
  layer gives it (`AppState.variables`, `AppState.manage`) and sends actions: the collection shown, the question before a
  removal, the rename field and the text of the three forms are the app layer's (finding T5), and so are the checks of a name
  and what a removal detaches. The ids are the app layer's too, so the thing that was added is found by `Event::EntityAdded` and given
  the focus. What this holds is drawing state only: the text of the last refusal or change (the live regions), the words of the
  change just sent, and where the focus goes back to. The words of a refusal are `notices.ts`, the others `manage.ts` and `rename.ts`.
-->
<script lang="ts">
  import { tick, untrack } from 'svelte';
  import type { Action, Event, ManageOp, ManageState, ManageTarget, VarType, VariableEntity, VariablesPanel } from '@meldframe/protocol';
  import Button from '@meldframe/web-chrome/Button.svelte';
  import CollectionDetail from './CollectionDetail.svelte';
  import CollectionList from './CollectionList.svelte';
  import RemoveConfirm from './RemoveConfirm.svelte';
  import { focusAsker as askerFocus, focusEntity as entityFocus } from './askers';
  import { collectionSummary, partial, reactTo, removalKey, removalWords, settle } from './manage';

  interface Props {
    panel: VariablesPanel;
    /** `AppState.manage`. */
    manage: ManageState;
    /** The events of the last dispatch (`snapshot.events`). */
    events: readonly Event[];
    dispatch: (a: Action) => void;
  }
  let { panel, manage, events, dispatch }: Props = $props();

  let root = $state<HTMLElement | null>(null);
  const current = $derived(panel.collections.find((c) => c.id === manage.shown) ?? null);
  const summary = $derived(current ? collectionSummary(current, panel) : null);
  const why = $derived(partial(panel));
  const removal = $derived(manage.removal ?? null);
  const words = $derived(removal ? removalWords(removal) : null);
  const rename = $derived(manage.rename ?? null);

  // The live regions are always on the page and the text is put into them (a region that appears with its text is not announced).
  let error = $state('');
  let status = $state('');
  // The words of the change just sent, said when the document answers; and the question to give the focus back to after it.
  let pending: string | null = null;
  // What was sent last: the words of a limit depend on it (`reactTo`).
  let sent: Action | null = null;
  let refocus: ManageTarget | null = null;
  // The last thing a field was open on: the button that opened it gets the focus back when the field closes.
  let lastRename: ManageTarget | null = null;
  $effect(() => {
    if (manage.rename) lastRename = manage.rename.target;
  });

  const focusEntity = (id: string) => entityFocus(root, id);
  /** Back to the button that asked; when that thing is gone, to the title of the collection, then to a collection, then to the heading of the list. */
  const focusAsker = (a: ManageTarget, attr: 'remove' | 'rename' = 'remove') => askerFocus(root, a, attr);

  const op = (o: ManageOp) => dispatch({ type: 'manage', op: o });
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
    const r = reactTo(now, untrack(() => pending), untrack(() => manage.limits), untrack(() => sent));
    const s = settle(r, lastRename, refocus);
    if (s.error !== null) error = s.error;
    if (s.status !== null) status = s.status;
    if (s.pending) pending = null;
    if (s.refocused) refocus = null;
    // The page shows the answer after this: what was added takes the focus, a field closed by its rename gives it back to the button that opened it, and a removal that was answered gives it back to the button that asked.
    if (s.focus.length) void tick().then(() => s.focus.forEach((f) => ('entity' in f ? focusEntity(f.entity) : focusAsker(f.asker, f.attr))));
  });

  const close = () => {
    const a = removal?.target;
    op({ type: 'cancelRemove' });
    if (a) void tick().then(() => focusAsker(a));
  };
  // A cancel is quiet: the words of an earlier change do not stay in the status line as if this had done something.
  const cancelRename = () => {
    const a = rename?.target;
    op({ type: 'cancelRename' });
    status = '';
    error = '';
    if (a) void tick().then(() => focusAsker(a, 'rename'));
  };
  const submitRename = () => {
    const quiet = rename?.unchanged;
    const a = rename?.target;
    pending = null;
    sent = null;
    op({ type: 'submitRename' });
    // A name that is no change closes the field and says nothing: a quiet cancel.
    if (quiet) {
      status = '';
      error = '';
      if (a) void tick().then(() => focusAsker(a, 'rename'));
    }
  };
  const confirm = () => {
    const w = words;
    refocus = removal?.target ?? null;
    pending = w?.done ?? null;
    sent = null;
    op({ type: 'confirmRemove' });
  };
  const draft = (form: VariableEntity, text: string) => op({ type: 'setDraft', form, text });
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
      <CollectionList
        collections={panel.collections}
        chosen={current?.id ?? null}
        onChoose={(id) => op({ type: 'choose', id })}
        draft={manage.drafts.collection}
        limits={manage.limits}
        onInput={(text) => draft('collection', text)}
        onAdd={(n) => send({ type: 'createCollection', name: n }, `Added the collection “${n}”.`)}
      />
    </div>
    <div class="min-w-0 flex-1 overflow-y-auto" data-testid="manage-detail">
      {#if removal && words}
        <!-- A question for another thing starts again, so it takes the focus on Cancel as the first one did. -->
        {#key removalKey(removal)}
          <div class="px-4 pt-4"><RemoveConfirm plan={words} onConfirm={confirm} onCancel={close} /></div>
        {/key}
      {/if}
      {#if current && summary}
        <CollectionDetail
          col={current}
          {summary}
          onAsk={(t) => op({ type: 'askRemove', target: t })}
          {rename}
          drafts={manage.drafts}
          limits={manage.limits}
          onRename={(t) => op({ type: 'startRename', target: t })}
          onRenameInput={(text) => op({ type: 'setRenameText', text })}
          onRenameSubmit={submitRename}
          onRenameCancel={cancelRename}
          onDraft={draft}
          onAddMode={(n, from) => send({ type: 'addMode', collection: current.id, name: n, ...(from ? { copyFrom: from } : {}) }, `Added the mode “${n}”.`)}
          onAddVariable={(n, kind: VarType) => send({ type: 'addVariable', collection: current.id, name: n, kind }, `Added the variable “${n}”.`)}
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
