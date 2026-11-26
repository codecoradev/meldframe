<!--
  The Activity panel (E5.1, ADR-0021): one lane per actor with its batches, newest first, then the checkpoints. It
  draws `state.activity` and the checkpoints of the app layer and sends actions: the lanes, their order, the counts
  and which row may be reverted are the app layer's, and so is the question being asked (`AppState.confirm`: revert a
  batch, restore a checkpoint). What it keeps itself is view state only (which lanes the person folded or opened), and
  every control sends one of the actions of actions.ts, which is tested.
-->
<script lang="ts">
  import { onDestroy, untrack } from 'svelte';
  import type { Action, ActivityState, ActorTone, Checkpoint, CheckpointsStatus, Confirm, Event } from '@meldframe/protocol';
  import Button from '@meldframe/web-chrome/Button.svelte';
  import Icon from '@meldframe/web-chrome/Icon.svelte';
  import Section from '@meldframe/web-chrome/Section.svelte';
  import { ring } from '@meldframe/web-chrome/focus';
  import { duration } from '@meldframe/web-tokens/tokens';
  import { askRestore, askRevert, cancel, confirmRestore, confirmRevert, createCheckpoint, older, setReview, show } from './actions';
  import BatchItem from './BatchItem.svelte';
  import CheckpointCreate from './CheckpointCreate.svelte';
  import CheckpointList from './CheckpointList.svelte';
  import { aboutAfter, eventsSay, laneOpen, start, toggleLane, type About, type Ui } from './controller';
  import MessageLine from './MessageLine.svelte';
  import { clockText, laneSummary, laneViews, rowKey, type Message } from './view';

  interface Props {
    activity: ActivityState;
    checkpoints: readonly Checkpoint[];
    /** Whether this caller may make a checkpoint (`AppState.canCreateCheckpoint`). */
    canCreate: boolean;
    /** Whether the list was read and whether the server keeps checkpoints (`AppState.checkpointsStatus`). */
    status: CheckpointsStatus;
    /** The actor of this page: its lane is called You. */
    self: string;
    /** The question being asked (`AppState.confirm`), or none. */
    confirm?: Confirm | null;
    dispatch: (a: Action) => void;
    /**
     * The events of the last dispatch. A refusal, a revert or a restore that comes while the panel is open is worded
     * (`messageFor`) and shown for a few seconds; what was already there when the panel opened is not.
     */
    events?: readonly Event[];
  }
  let { activity, checkpoints, canCreate, status, self, confirm = null, dispatch, events = [] }: Props = $props();

  const lanes = $derived(laneViews(activity, self));
  let ui = $state.raw<Ui>(start);
  // What the person asked for last decides the words of a refusal: `forbidden` means one thing for a review and another for a checkpoint.
  let about = $state.raw<About>(undefined);
  const send = (a: Action) => {
    about = aboutAfter(about, a);
    dispatch(a);
  };
  // How many checkpoints this person asked for were made (the app layer says so by `Event.checkpointMade`): the name field empties.
  let made = $state(0);

  let message = $state<Message | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let seen = untrack(() => events);
  $effect(() => {
    const now = events;
    if (now === seen) return;
    seen = now;
    const says = eventsSay(now, about);
    if (says.made) made += 1;
    if (says.message) say(says.message);
  });
  function say(m: Message) {
    message = m;
    clearTimeout(timer);
    timer = setTimeout(() => (message = null), duration.notice);
  }
  onDestroy(() => clearTimeout(timer));

  // Presentation only: the tone is the app layer's (`ActorView.tone`), this is the class that draws it.
  const tone: Record<ActorTone, string> = { person: 'bg-person', agent1: 'bg-agent-1', agent2: 'bg-agent-2', agent3: 'bg-agent-3' };
  const changes = (n: number) => `${n} ${n === 1 ? 'change' : 'changes'}`;
  const asking = (r: { actor: string; keyPrefix: string }) => confirm?.kind === 'revert' && confirm.actor === r.actor && confirm.keyPrefix === r.keyPrefix;
</script>

<div class="flex flex-col" data-testid="activity">
  <MessageLine {message} />
  <Section title="Recent work">
    {#if activity.unreviewedCount === null}
      <p class="m-0 text-small text-text-muted" data-testid="review-count" data-unknown="true">Batches to review: Unknown. The list of batches has not loaded.</p>
    {:else if activity.unreviewedCount > 0}
      <p class="m-0 text-small text-text-secondary" data-testid="review-count">{activity.unreviewedCount} {activity.unreviewedCount === 1 ? 'batch' : 'batches'} to review</p>
    {/if}
    {#if lanes.length === 0}
      <p class="m-0 text-small text-text-muted" data-testid="activity-empty">Nothing yet. When an agent or a person changes the file, the work shows here, one lane for each.</p>
    {/if}
    {#each lanes as l, i (l.actor)}
      {@const open = laneOpen(ui, l.actor, i)}
      <div class="flex flex-col gap-2" data-testid="lane" data-actor={l.actor} data-state={l.stateWords.toLowerCase()}>
        <button
          type="button"
          aria-expanded={open}
          aria-controls="lane-body-{i}"
          onclick={() => (ui = toggleLane(ui, l.actor, open))}
          class="{ring} flex min-h-(--mf-size-control-lg) w-full flex-col justify-center rounded-md py-1 text-left hov:bg-control"
        >
          <span class="flex w-full items-center gap-2">
            <span class="text-icon-muted"><Icon name={open ? 'chevron-down' : 'chevron-right'} /></span>
            <span class="size-2 shrink-0 rounded-pill {tone[l.tone]}" data-tone={l.tone} aria-hidden="true"></span>
            <span class="min-w-0 flex-1 truncate text-body font-semibold text-text-strong" title={l.actor}><bdi>{l.name}</bdi></span>
          </span>
          <span class="pl-12 text-caption text-text-muted" data-testid="lane-summary">{laneSummary(l)}</span>
        </button>
        {#if open}
          <div id="lane-body-{i}" class="flex flex-col gap-2">
            {#if l.rows.length}
              <ul class="m-0 flex list-none flex-col gap-2 p-0">
                {#each l.rows as r (rowKey(r.actor, r.keyPrefix))}
                  <BatchItem
                    row={r}
                    selected={activity.selected?.actor === r.actor && activity.selected.keyPrefix === r.keyPrefix}
                    confirming={asking(r)}
                    canReview={activity.canReview}
                    onReview={(state) => dispatch(setReview(r, state))}
                    onShow={() => send(show(r))}
                    onAskRevert={() => send(askRevert(r))}
                    onRevert={() => send(confirmRevert(r))}
                    onCancel={() => send(cancel())}
                  />
                {/each}
              </ul>
            {:else}
              <ul class="m-0 flex list-none flex-col gap-1 p-0 pl-6 text-small text-text-secondary" data-testid="lane-runs">
                {#each l.runs as stretch (stretch.startMs)}
                  <li>{clockText(stretch.startMs)} to {clockText(stretch.endMs)}: {changes(stretch.ops)}</li>
                {/each}
              </ul>
            {/if}
          </div>
        {/if}
      </div>
    {/each}
    {#if activity.hasOlder}
      <Button size="sm" onclick={() => dispatch(older())} data-testid="older">Show older batches</Button>
    {/if}
  </Section>
  <Section title="Checkpoints">
    <CheckpointCreate {canCreate} {status} {made} onCreate={(label) => send(createCheckpoint(label))} />
    <CheckpointList
      {checkpoints}
      asking={confirm?.kind === 'restore' ? confirm.id : null}
      onAsk={(id) => send(askRestore(id))}
      onRestore={(id) => send(confirmRestore(id))}
      onCancel={() => send(cancel())}
    />
  </Section>
</div>
