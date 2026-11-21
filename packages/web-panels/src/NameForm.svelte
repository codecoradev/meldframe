<!--
  A field with an Add button: the name of something new. The name is checked as typed (`checkName`, the core's rule
  of the kind in the field's own words); Add is off while it cannot be sent, and a name the core would accept but that
  exists already is a warning. Enter adds (a Field would take Enter as "done", so it is read before the field sees it).
  What is typed is the only state here; a parent starts the form again (`{#key}`) once the thing exists, and keeps the
  text while the change is refused, so the person can correct it.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import Button from '@meldframe/web-chrome/Button.svelte';
  import Field from '@meldframe/web-chrome/Field.svelte';
  import { NAME_RULE_WORDS } from '@meldframe/web-document/notices';
  import { checkName, type NameKind } from './manage';

  interface Props {
    kind: NameKind;
    /** The accessible name of the field. */
    label: string;
    /** The text of the button. */
    submit: string;
    /** The names that exist now (in the file for a collection, in the collection for a mode or a variable). */
    existing: readonly string[];
    placeholder?: string;
    /** Controls after the field and before the button (the type, where the values come from). */
    extra?: Snippet;
    onSubmit: (name: string) => void;
    /** `data-testid` of the field; the button's is the same with `-add` after it and the note's with `-note`. */
    testId: string;
  }
  let { kind, label, submit, existing, placeholder, extra, onSubmit, testId }: Props = $props();

  let text = $state('');
  const check = $derived(checkName(kind, text, existing));
  const touched = $derived(text.trim() !== '');
  const note = $derived.by(() => {
    if (touched && check.error) return { tone: 'error', text: check.error };
    if (check.warning) return { tone: 'warning', text: check.warning };
    return kind === 'mode' ? null : { tone: 'hint', text: NAME_RULE_WORDS };
  });
  const send = () => {
    if (!check.error) onSubmit(check.name);
  };
  const noteClass = { error: 'text-danger-text', warning: 'text-text-strong', hint: 'text-text-muted' } as const;
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
    <div class="min-w-40 flex-1"><Field {label} size="dense" value={text} onInput={(v) => (text = v)} invalid={touched && !!check.error} {placeholder} {testId} /></div>
    {@render extra?.()}
    <Button variant="primary" size="sm" disabled={!!check.error} onclick={send} data-testid="{testId}-add">{submit}</Button>
  </div>
  <p class="m-0 min-h-4 text-caption {note ? noteClass[note.tone as keyof typeof noteClass] : ''}" data-testid="{testId}-note" data-tone={note?.tone}>{note?.text ?? ''}</p>
</div>
