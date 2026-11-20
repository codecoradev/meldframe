<!--
  The Activity panel (E5.1, ADR-0021): one lane per actor with its batches, newest first, then the checkpoints. It
  draws `state.activity` and the checkpoints of the app layer and sends actions: the lanes, their order, the counts
  and which row may be reverted are the app layer's. What it keeps itself is view state only (which lanes the person
  folded or opened, and which confirmation is open), and every click goes through controller.ts, which is tested.
-->
<script lang="ts">
  import { onDestroy, untrack } from 'svelte';
  import type { Action, ActivityState, Checkpoint, Event } from '@meldframe/protocol';
  import Button from '@meldframe/web-chrome/Button.svelte';
  import Icon from '@meldframe/web-chrome/Icon.svelte';
  import Section from '@meldframe/web-chrome/Section.svelte';
  import { ring } from '@meldframe/web-chrome/focus';
  import { duration } from '@meldframe/web-tokens/tokens';
  import { older } from './actions';
  import BatchItem from './BatchItem.svelte';
  import CheckpointList from './CheckpointList.svelte';
  import { askRestore, askRevert, cancel, confirmRestore, confirmRevert, laneOpen, settle, show, start, toggleLane, type Step, type Ui } from './controller';
  import MessageLine from './MessageLine.svelte';
  import { clockText, laneSummary, laneViews, messageFor, type Message } from './view';

  interface Props {
    activity: ActivityState;
    checkpoints: readonly Checkpoint[];
    /** The actor of this page: its lane is called You. */
    self: string;
    dispatch: (a: Action) => void;
    /**
     * The events of the last dispatch. A refusal, a revert or a restore that comes while the panel is open is worded
     * (`messageFor`) and shown for a few seconds; what was already there when the panel opened is not.
     */
    events?: readonly Event[];
  }
  let { activity, checkpoints, self, dispatch, events = [] }: Props = $props();

  const lanes = $derived(laneViews(activity, self));
  let ui = $state.raw<Ui>(start);
  const run = (s: Step) => {
    ui = s.ui;
    for (const a of s.actions) dispatch(a);
  };
  // A confirmation belongs to a row that is still there and still selected, and to a checkpoint that still exists.
  $effect(() => {
    const next = settle(ui, activity, checkpoints);
    if (next !== ui) ui = next;
  });

  let message = $state<Message | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let seen = untrack(() => events);
  $effect(() => {
    const now = events;
    if (now === seen) return;
    seen = now;
    const m = messageFor(now);
    if (!m) return;
    message = m;
    clearTimeout(timer);
    timer = setTimeout(() => (message = null), duration.notice);
  });
  onDestroy(() => clearTimeout(timer));

  const key = (actor: string, prefix: string) => `${actor}\u0000${prefix}`;
  const tone = { 1: 'bg-agent-1', 2: 'bg-agent-2', 3: 'bg-agent-3' } as const;
  const changes = (n: number) => `${n} ${n === 1 ? 'change' : 'changes'}`;
  const asking = (r: { actor: string; keyPrefix: string }) => ui.confirm?.kind === 'revert' && ui.confirm.actor === r.actor && ui.confirm.keyPrefix === r.keyPrefix;
</script>

<div class="flex flex-col" data-testid="activity">
  <MessageLine {message} />
  <Section title="Recent work">
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
          onclick={() => run(toggleLane(ui, l.actor, open))}
          class="{ring} flex min-h-(--mf-size-control-lg) w-full flex-col justify-center rounded-md py-1 text-left hov:bg-control"
        >
          <span class="flex w-full items-center gap-2">
            <span class="text-icon-muted"><Icon name={open ? 'chevron-down' : 'chevron-right'} /></span>
            <span class="size-2 shrink-0 rounded-pill {l.tone ? tone[l.tone] : 'bg-person'}" aria-hidden="true"></span>
            <span class="min-w-0 flex-1 truncate text-body font-semibold text-text-strong" title={l.actor}><bdi>{l.name}</bdi></span>
          </span>
          <span class="pl-12 text-caption text-text-muted" data-testid="lane-summary">{laneSummary(l)}</span>
        </button>
        {#if open}
          <div id="lane-body-{i}" class="flex flex-col gap-2">
            {#if l.rows.length}
              <ul class="m-0 flex list-none flex-col gap-2 p-0">
                {#each l.rows as r (key(r.actor, r.keyPrefix))}
                  <BatchItem
                    row={r}
                    selected={activity.selected?.actor === r.actor && activity.selected.keyPrefix === r.keyPrefix}
                    confirming={asking(r)}
                    onShow={() => run(show(ui, r))}
                    onAskRevert={() => run(askRevert(ui, r))}
                    onRevert={() => run(confirmRevert(ui, r, activity.selected))}
                    onCancel={() => run(cancel(ui))}
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
    <CheckpointList
      {checkpoints}
      asking={ui.confirm?.kind === 'restore' ? ui.confirm.id : null}
      onAsk={(id) => run(askRestore(ui, id))}
      onRestore={(id) => run(confirmRestore(ui, id))}
      onCancel={() => run(cancel(ui))}
    />
  </Section>
</div>
