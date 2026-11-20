<!--
  The checkpoints of this page, newest first, each with Restore behind a confirmation. A checkpoint is the file as the
  server confirmed it after change `seq`; a restore adds new changes at the end of the log and never rewrites it. Which
  checkpoint is being asked about is the panel's (controller.ts): this draws it and moves the keyboard focus.
-->
<script lang="ts">
  import { tick } from 'svelte';
  import Button from '@meldframe/web-chrome/Button.svelte';
  import type { Checkpoint } from '@meldframe/protocol';
  import { checkpointWords } from './view';

  interface Props {
    checkpoints: readonly Checkpoint[];
    /** The id of the checkpoint a Restore is being confirmed for. */
    asking: string | null;
    onAsk: (id: string) => void;
    onRestore: (id: string) => void;
    onCancel: () => void;
  }
  let { checkpoints, asking, onAsk, onRestore, onCancel }: Props = $props();
  let list = $state<HTMLElement | null>(null);
  // Newest first: the app keeps them oldest first.
  const shown = $derived([...checkpoints].reverse());

  const button = (id: string, testid: string) => list?.querySelector<HTMLElement>(`[data-checkpoint="${CSS.escape(id)}"] [data-testid="${testid}"] button`);
  // The question takes the focus (on its safe button), and Restore gets it back when the question closes.
  let before: string | null = null;
  $effect(() => {
    const now = asking;
    const was = before;
    before = now;
    if (now !== null) void tick().then(() => button(now, 'restore-cancel')?.focus());
    else if (was !== null) void tick().then(() => button(was, 'checkpoint-restore')?.focus());
  });
</script>

<div bind:this={list} data-testid="checkpoints">
  {#if shown.length === 0}
    <p class="m-0 text-small text-text-muted">No checkpoint yet. One is made before an agent's run starts, and before a revert or a restore.</p>
  {:else}
    <ul class="m-0 flex list-none flex-col gap-2 p-0">
      {#each shown as c (c.id)}
        <li class="flex flex-col gap-2 rounded-md border border-hairline bg-field px-3 py-2" data-testid="checkpoint" data-checkpoint={c.id}>
          <div class="flex items-baseline justify-between gap-2">
            <span class="min-w-0 break-words text-small font-medium text-text-strong"><bdi>{checkpointWords(c)}</bdi></span>
            <span class="shrink-0 text-caption text-text-muted">After change {c.seq}</span>
          </div>
          {#if asking === c.id}
            <!-- Escape closes the question from either button; the group is not a control itself. -->
            <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
            <div
              role="group"
              aria-label="Confirm the restore of this checkpoint"
              class="flex flex-col gap-2"
              data-testid="restore-confirm"
              onkeydown={(e) => { if (e.key === 'Escape') { e.stopPropagation(); onCancel(); } }}
            >
              <p class="m-0 text-small text-text">Restore the file to this point? New changes at the end of the log put it back; nothing is rewritten.</p>
              <div class="flex gap-2">
                <Button size="sm" variant="destructive" onclick={() => onRestore(c.id)} data-testid="restore-yes">Restore</Button>
                <span data-testid="restore-cancel" class="contents"><Button size="sm" onclick={onCancel}>Cancel</Button></span>
              </div>
            </div>
          {:else}
            <span class="self-start" data-testid="checkpoint-restore"><Button size="sm" onclick={() => onAsk(c.id)}>Restore</Button></span>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>
