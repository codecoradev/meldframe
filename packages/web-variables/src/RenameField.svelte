<!--
  The field that takes the place of a name while it is renamed (the Rename button of a collection, a mode or a variable
  opens it): prefilled with the name as stored, selected, with Save and Cancel. Enter saves and Escape cancels, and
  only the edit: the key stops here, so the dialog around stays open. The field is app state (`AppState.manage.rename`): its
  text, what the name comes to (the rules of an addition, without the own name among the names taken; a text that is the name it
  has now or the one the field opened with is no change, so an untouched field never undoes the rename of another actor), and
  what changed under it. Save is off while the name cannot be sent; a duplicate collection or mode is a warning. The text is sent as
  typed; the app layer closes the field once the change was made and leaves it open (with its text) while the change is refused, so the
  person can correct it. A refusal is shown by the parent. The text is the raw name: it is the field's value, never markup.
-->
<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import type { ManageLimits, RenameField as RenameState } from '@meldframe/protocol';
  import Button from '@meldframe/web-chrome/Button.svelte';
  import Field from '@meldframe/web-chrome/Field.svelte';
  import { NOTE_CLASS, type NameKind } from './manage';
  import { movedWords, renameNote } from './rename';

  interface Props {
    kind: NameKind;
    /** What the target is called in a name of a control: `color/bg`, `collection theme`, `mode dark` (display names). */
    subject: string;
    /** The field as the app layer holds it. */
    field: RenameState | null;
    limits: ManageLimits;
    onInput: (text: string) => void;
    onSubmit: () => void;
    onCancel: () => void;
    /** `data-testid` of the field; the buttons' are the same with `-save` and `-cancel` after it, the note's with `-note`. */
    testId: string;
  }
  let { kind, subject, field, limits, onInput, onSubmit, onCancel, testId }: Props = $props();

  let root = $state<HTMLElement | null>(null);
  // The parent draws this only while the field is open, but its prop turns null a moment before the field is torn down: keep the last one to draw until then.
  let last = untrack(() => field) as RenameState;
  const f = $derived.by(() => {
    if (field) last = field;
    return last;
  });
  const check = $derived(f.check);
  const changed = $derived(!f.unchanged);
  const moved = $derived(movedWords(f.opened, f.current));
  const note = $derived(renameNote(kind, f, limits));
  const save = () => {
    if (!check.error) onSubmit();
  };
  onMount(() => {
    const input = root?.querySelector('input');
    input?.focus();
    input?.select();
  });
</script>

<!-- The keys are read in the capture phase: the Field would otherwise end the edit on Enter and Escape and the key would never reach the form. -->
<div
  bind:this={root}
  class="flex min-w-0 flex-col gap-1"
  data-testid="{testId}-form"
  onkeydowncapture={(e) => {
    if (e.isComposing) return;
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      onCancel();
    } else if (e.key === 'Enter' && (e.target as HTMLElement).tagName === 'INPUT') {
      e.preventDefault();
      e.stopPropagation();
      save();
    }
  }}
>
  <div class="flex flex-wrap items-center gap-2">
    <div class="min-w-40 flex-1"><Field label="New name of {subject}" size="dense" value={f.text} {onInput} invalid={changed && !!check.error} {testId} /></div>
    <Button variant="primary" size="sm" aria-label="Save the new name of {subject}" disabled={!!check.error} onclick={save} data-testid="{testId}-save">Save</Button>
    <Button size="sm" aria-label="Cancel renaming {subject}" onclick={onCancel} data-testid="{testId}-cancel">Cancel</Button>
  </div>
  <p class="m-0 text-caption whitespace-normal {NOTE_CLASS[note.tone]}" data-testid="{testId}-note" data-tone={note.tone}>{note.text}</p>
  <!-- Always on the page, so the words are announced when they are put in. -->
  <p role="status" class="m-0 text-caption whitespace-normal text-text-strong" data-testid="{testId}-moved">{moved ?? ''}</p>
</div>
