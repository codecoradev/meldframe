<!--
  One batch of a lane: its number, how far it got, the time, what it did, the agent's note, and Show and Revert. The
  note is text an agent wrote: it is drawn as text only (no markup, no links), whole (at most 500 characters and 3 line
  breaks, so it cannot take over the card), attributed to its full actor and in quotation marks, and a bidirectional
  override inside it cannot reach the text around it (<bdi>). Which row may be reverted, and whether a revert would
  still change anything, is what the app layer says (`revertable`, `inEffect`). What a click does is in controller.ts.
-->
<script lang="ts">
  import { tick } from 'svelte';
  import Button from '@meldframe/web-chrome/Button.svelte';
  import type { BatchRow } from '@meldframe/protocol';
  import { batchHeading, clockText, statusWords, summaryWords } from './view';

  interface Props {
    row: BatchRow;
    selected: boolean;
    /** The confirmation of a revert is open on this row. */
    confirming: boolean;
    onShow: () => void;
    onAskRevert: () => void;
    onRevert: () => void;
    onCancel: () => void;
  }
  let { row, selected, confirming, onShow, onAskRevert, onRevert, onCancel }: Props = $props();

  const heading = $derived(batchHeading(row));
  const summary = $derived(summaryWords(row.summary));
  let revertButton = $state<HTMLElement | null>(null);
  let cancelButton = $state<HTMLElement | null>(null);

  // Keyboard: the confirmation takes the focus (on its safe button) and gives it back to Revert when it closes.
  $effect(() => {
    if (confirming) void tick().then(() => cancelButton?.querySelector('button')?.focus());
  });
  const close = (then: () => void) => () => {
    then();
    void tick().then(() => revertButton?.querySelector('button')?.focus());
  };
</script>

<li
  class="flex flex-col gap-2 rounded-md border px-3 py-2 {selected ? 'border-selection bg-selection-bg' : 'border-hairline bg-field'}"
  data-testid="batch"
  data-n={row.n ?? undefined}
  data-status={row.status}
  aria-current={selected ? 'true' : undefined}
>
  <div class="flex items-baseline justify-between gap-2">
    <span class="text-body font-semibold text-text-strong">{heading}</span>
    <span class="text-caption text-text-muted">{clockText(row.atMs)}</span>
  </div>
  <p class="m-0 text-small text-text-secondary" data-testid="batch-status">{statusWords(row)}</p>
  {#if summary}<p class="m-0 text-small text-text break-words" data-testid="batch-summary">{summary}</p>{/if}
  {#if row.note}
    <figure class="m-0 flex flex-col gap-1" data-testid="batch-note">
      <figcaption class="text-caption text-text-muted break-words">Agent note from <bdi>{row.actor}</bdi></figcaption>
      <blockquote class="m-0 border-l-2 border-border-strong pl-2 text-small text-text break-words whitespace-pre-line">“<bdi>{row.note}</bdi>”</blockquote>
    </figure>
  {/if}
  <div class="flex flex-wrap items-center gap-2">
    <Button size="sm" disabled={!row.canShow} onclick={onShow} data-testid="batch-show">Show</Button>
    {#if row.revertable}
      <span bind:this={revertButton} class="contents"><Button size="sm" variant="secondary" onclick={onAskRevert} data-testid="batch-revert">Revert</Button></span>
    {:else if row.n !== null}
      <span class="text-caption text-text-muted" data-testid="batch-not-revertable">Cannot be reverted from this page.</span>
    {/if}
  </div>
  {#if confirming}
    <!-- Escape closes the confirmation from any control inside it; the group is not a control itself. -->
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
      role="group"
      aria-label="Confirm the revert of {heading}"
      class="flex flex-col gap-2 rounded-md border border-border bg-surface px-3 py-2"
      data-testid="revert-confirm"
      onkeydown={(e) => { if (e.key === 'Escape') { e.stopPropagation(); close(onCancel)(); } }}
    >
      {#if row.inEffect === false}
        <p class="m-0 text-small text-text">Nothing left to revert: every field this batch wrote was written again since, or it was reverted already.</p>
        <span bind:this={cancelButton} class="contents"><Button size="sm" onclick={close(onCancel)} data-testid="revert-cancel">Close</Button></span>
      {:else}
        <p class="m-0 text-small text-text">Revert {heading}? What its agent wrote last goes back, as a new batch of yours. What others wrote since stays.</p>
        <div class="flex gap-2">
          <Button size="sm" variant="destructive" onclick={close(onRevert)} data-testid="revert-confirm-yes">Revert</Button>
          <span bind:this={cancelButton} class="contents"><Button size="sm" onclick={close(onCancel)} data-testid="revert-cancel">Cancel</Button></span>
        </div>
      {/if}
    </div>
  {/if}
</li>
