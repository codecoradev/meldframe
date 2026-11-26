<!--
  A field with an Add button: the name of something new. The text and what it comes to are the app layer's
  (`AppState.manage.drafts`: the core's rule of the kind, the limits, the names it is compared with); this draws the check
  in the field's own words. Add is off while the name cannot be sent, and a name the core would accept but that exists already is a
  warning. Enter adds (a Field would take Enter as "done", so it is read before the field sees it). Nothing is held here:
  the text is sent as typed, and the app layer empties it once the thing exists and keeps it while the change is refused, so the person
  can correct it.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { ManageLimits, NameDraft } from '@meldframe/protocol';
  import Button from '@meldframe/web-chrome/Button.svelte';
  import Field from '@meldframe/web-chrome/Field.svelte';
  import { NOTE_CLASS, addNote, type NameKind, typed } from './manage';

  interface Props {
    kind: NameKind;
    /** The accessible name of the field. */
    label: string;
    /** The text of the button. */
    submit: string;
    /** The text of the form and its check (`AppState.manage.drafts`). */
    draft: NameDraft;
    limits: ManageLimits;
    placeholder?: string;
    /** Controls after the field and before the button (the type, where the values come from). */
    extra?: Snippet;
    onInput: (text: string) => void;
    onSubmit: (name: string) => void;
    /** `data-testid` of the field; the button's is the same with `-add` after it and the note's with `-note`. */
    testId: string;
  }
  let { kind, label, submit, draft, limits, placeholder, extra, onInput, onSubmit, testId }: Props = $props();

  const check = $derived(draft.check);
  const touched = $derived(typed(draft.text));
  const note = $derived(addNote(kind, draft, limits));
  const send = () => {
    if (!check.error) onSubmit(check.name);
  };
</script>

<!-- Enter is read in the capture phase: the Field would otherwise end the edit and the key never reach the form. -->
<div
  class="flex flex-col gap-1"
  onkeydowncapture={(e) => {
    if (e.key !== 'Enter' || e.isComposing) return;
    e.preventDefault();
    e.stopPropagation();
    send();
  }}
>
  <div class="flex flex-wrap items-center gap-2">
    <div class="min-w-40 flex-1"><Field {label} size="dense" value={draft.text} {onInput} invalid={touched && !!check.error} {placeholder} {testId} /></div>
    {@render extra?.()}
    <Button variant="primary" size="sm" disabled={!!check.error} onclick={send} data-testid="{testId}-add">{submit}</Button>
  </div>
  <p class="m-0 min-h-4 text-caption {note ? NOTE_CLASS[note.tone] : ''}" data-testid="{testId}-note" data-tone={note?.tone}>{note?.text ?? ''}</p>
</div>
