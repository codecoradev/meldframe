// React copy of NameForm.svelte. A field with an Add button: the name of something new. The text and what it comes to are the
// app layer's (`AppState.manage.drafts`: the core's rule of the kind, the limits, the names it is compared with); this draws the
// check in the field's own words (`addNote`, shared with the Svelte copy). Add is off while the name cannot be sent, and a name the
// core would accept but that exists already is a warning. Enter adds (a Field would take Enter as "done", so it is read in the
// capture phase, before the field sees it). Nothing is held here: the text is sent as typed.
import type { ManageLimits, NameDraft } from '@meldframe/protocol';
import { Button } from '@meldframe/web-chrome/react/Button';
import { Field } from '@meldframe/web-chrome/react/Field';
import type { ReactNode } from 'react';
import { NOTE_CLASS, type NameKind, addNote, typed } from '../manage';

export interface NameFormProps {
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
  extra?: ReactNode;
  onInput: (text: string) => void;
  onSubmit: (name: string) => void;
  /** `data-testid` of the field; the button's is the same with `-add` after it and the note's with `-note`. */
  testId: string;
}

export function NameForm({ kind, label, submit, draft, limits, placeholder, extra, onInput, onSubmit, testId }: NameFormProps) {
  const check = draft.check;
  const touched = typed(draft.text);
  const note = addNote(kind, draft, limits);
  const send = () => {
    if (!check.error) onSubmit(check.name);
  };
  return (
    // Enter is read in the capture phase: the Field would otherwise end the edit and the key never reach the form.
    <div
      className="flex flex-col gap-1"
      onKeyDownCapture={(e) => {
        if (e.key !== 'Enter' || e.nativeEvent.isComposing) return;
        e.preventDefault();
        e.stopPropagation();
        send();
      }}
    >
      <div className="flex flex-wrap items-center gap-2">
        <div className="min-w-40 flex-1">
          <Field label={label} size="dense" value={draft.text} onInput={onInput} invalid={touched && !!check.error} placeholder={placeholder} testId={testId} />
        </div>
        {extra}
        <Button variant="primary" size="sm" disabled={!!check.error} onClick={send} data-testid={`${testId}-add`}>
          {submit}
        </Button>
      </div>
      <p className={`m-0 min-h-4 text-caption ${note ? NOTE_CLASS[note.tone] : ''}`} data-testid={`${testId}-note`} data-tone={note?.tone}>
        {note?.text ?? ''}
      </p>
    </div>
  );
}
