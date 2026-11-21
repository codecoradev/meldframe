<!--
  The field that takes the place of a name while it is renamed (the Rename button of a collection, a mode or a variable
  opens it): prefilled with the name as stored, selected, with Save and Cancel. Enter saves and Escape cancels, and
  only the edit: the key stops here, so the dialog around stays open. A name that is the one it has now is a quiet
  cancel: nothing is sent. The name is checked as typed (`checkRename`: the rules of an addition, without the own name
  among the names taken; the text it opened with is no change either, so an untouched field never undoes the rename of
  another actor, and the field says when the name changed under it); Save is off while it cannot be sent, a duplicate collection or mode is a warning. What is
  typed is the only state here (and the text it opened with, to tell an untouched field from an edited one); the parent keeps the target as an id and closes the field once the change was made, and
  leaves it open (with its text) while the change is refused, so the person can correct it. A refusal is shown by the
  parent. The text is the raw name: it is the field's value, never markup.
-->
<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import Button from '@meldframe/web-chrome/Button.svelte';
  import Field from '@meldframe/web-chrome/Field.svelte';
  import { NAME_RULE_WORDS } from '@meldframe/web-document/notices';
  import type { NameKind } from './manage';
  import { checkRename, movedWords, renameHint, type RenameContext } from './rename';

  interface Props {
    kind: NameKind;
    /** What the target is called in a name of a control: `color/bg`, `collection theme`, `mode dark` (display names). */
    subject: string;
    /** The name it has now and the names of the others, read again from the panel on every change. */
    ctx: RenameContext;
    /** The target is the first (default) mode of its collection: the export does not name it. */
    first?: boolean;
    onSubmit: (name: string) => void;
    onCancel: () => void;
    /** `data-testid` of the field; the buttons' are the same with `-save` and `-cancel` after it, the note's with `-note`. */
    testId: string;
  }
  let { kind, subject, ctx, first = false, onSubmit, onCancel, testId }: Props = $props();

  let root = $state<HTMLElement | null>(null);
  // The name as the field opened: what is typed is a change only when it differs from this and from the name now.
  const opened = untrack(() => ctx.current);
  let text = $state(opened);
  const check = $derived(checkRename(kind, text, ctx, opened));
  const changed = $derived(!check.unchanged);
  const moved = $derived(movedWords(opened, ctx));
  const note = $derived.by(() => {
    if (changed && check.error) return { tone: 'error', text: check.error };
    if (check.warning) return { tone: 'warning', text: check.warning };
    return { tone: 'hint', text: kind === 'mode' ? renameHint(kind, first) : `${NAME_RULE_WORDS} ${renameHint(kind, first)}` };
  });
  const noteClass = { error: 'text-danger-text', warning: 'text-text-strong', hint: 'text-text-muted' } as const;
  const save = () => {
    if (check.error) return;
    if (check.unchanged) onCancel();
    else onSubmit(check.name);
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
    <div class="min-w-40 flex-1"><Field label="New name of {subject}" size="dense" value={text} onInput={(v) => (text = v)} invalid={changed && !!check.error} {testId} /></div>
    <Button variant="primary" size="sm" aria-label="Save the new name of {subject}" disabled={!!check.error} onclick={save} data-testid="{testId}-save">Save</Button>
    <Button size="sm" aria-label="Cancel renaming {subject}" onclick={onCancel} data-testid="{testId}-cancel">Cancel</Button>
  </div>
  <p class="m-0 text-caption whitespace-normal {noteClass[note.tone as keyof typeof noteClass]}" data-testid="{testId}-note" data-tone={note.tone}>{note.text}</p>
  <!-- Always on the page, so the words are announced when they are put in. -->
  <p role="status" class="m-0 text-caption whitespace-normal text-text-strong" data-testid="{testId}-moved">{moved ?? ''}</p>
</div>
