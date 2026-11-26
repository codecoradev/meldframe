<!--
  Create checkpoint (ADR-0023): a text field for the name and a button. The server keeps the row, so nothing here
  changes the list: the checkpoint shows when the server has made it. Where the caller may not make one the button is
  disabled and the field says why in text (a list that has not loaded and a server with no checkpoints are not the
  person's doing), and Enter in the field goes through the same step (controller.ts) as the button. The name is plain
  text; the app layer checks it again and a refusal is worded in the panel's status line. The field stops typing at 128
  UTF-16 units, which is not the rule (128 characters, as the server counts them): the app's refusal is the exact check.
  The field empties only when the app layer says the row this request asked for is there (`Event.checkpointMade`, told by the
  request key), so a name being corrected is not cleared by an agent's run start or another person's row, and a refusal keeps
  the text. The panel counts those events in `made`.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import type { CheckpointsStatus } from '@meldframe/protocol';
  import Button from '@meldframe/web-chrome/Button.svelte';
  import { fieldHeight, ringWithin } from '@meldframe/web-chrome/focus';
  import { isSubmitKey } from './controller';
  import { createWhyNot } from './view';

  interface Props {
    canCreate: boolean;
    status: CheckpointsStatus;
    /** How many checkpoints this person asked for were made: it grows when one arrives, and the field empties. */
    made: number;
    /** Makes the checkpoint with the text of the field. */
    onCreate: (label: string) => void;
  }
  let { canCreate, status, made, onCreate }: Props = $props();
  const uid = $props.id();
  const why = $derived(createWhyNot(canCreate, status));
  let text = $state('');
  let seen = untrack(() => made);
  $effect(() => {
    if (made === seen) return;
    seen = made;
    text = '';
  });
  const submit = () => {
    if (!canCreate) return;
    onCreate(text);
  };
</script>

<div class="flex flex-col gap-2" data-testid="checkpoint-new">
  <label for="{uid}-name" class="text-small text-text-secondary">Name (optional)</label>
  <div class="flex items-center gap-2">
    <input
      id="{uid}-name"
      data-testid="checkpoint-name"
      type="text"
      maxlength="128"
      autocomplete="off"
      bind:value={text}
      disabled={!canCreate}
      aria-describedby={why ? `${uid}-why` : undefined}
      onkeydown={(e) => {
        if (isSubmitKey(e)) {
          e.preventDefault();
          submit();
        }
      }}
      class="{fieldHeight.default} {ringWithin} min-w-0 flex-1 rounded-md border border-divider bg-field px-3 text-small text-text hov:border-border-strong"
    />
    <Button size="sm" onclick={submit} disabled={!canCreate} aria-describedby={why ? `${uid}-why` : undefined} data-testid="checkpoint-create">Create checkpoint</Button>
  </div>
  {#if why}
    <p id="{uid}-why" class="m-0 text-caption text-text-muted" data-testid="checkpoint-create-why">{why}</p>
  {/if}
</div>
