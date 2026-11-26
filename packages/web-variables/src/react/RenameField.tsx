// React copy of RenameField.svelte. The field that takes the place of a name while it is renamed (the Rename button of a collection,
// a mode or a variable opens it): prefilled with the name as stored, selected, with Save and Cancel. Enter saves and Escape
// cancels, and only the edit: the key stops here, so the dialog around stays open. The field is app state
// (`AppState.manage.rename`): its text, what the name comes to, whether the text is no change, and what changed under it. Save is
// off while the name cannot be sent; a duplicate collection or mode is a warning. The text is sent as typed; the app layer closes
// the field once the change was made and leaves it open (with its text) while the change is refused. A refusal is shown by the
// parent. The text is the raw name: it is the field's value, never markup. The parent draws this only while the field is open, so
// `field` is never null here (the Svelte copy keeps the last one for the moment between the prop turning null and the teardown).
import type { ManageLimits, RenameField as RenameState } from '@meldframe/protocol';
import { Button } from '@meldframe/web-chrome/react/Button';
import { Field } from '@meldframe/web-chrome/react/Field';
import { useEffect, useRef } from 'react';
import { NOTE_CLASS, type NameKind } from '../manage';
import { movedWords, renameNote } from '../rename';

export interface RenameFieldProps {
  kind: NameKind;
  /** What the target is called in a name of a control: `color/bg`, `collection theme`, `mode dark` (display names). */
  subject: string;
  /** The field as the app layer holds it. */
  field: RenameState;
  limits: ManageLimits;
  onInput: (text: string) => void;
  onSubmit: () => void;
  onCancel: () => void;
  /** `data-testid` of the field; the buttons' are the same with `-save` and `-cancel` after it, the note's with `-note`. */
  testId: string;
}

export function RenameField({ kind, subject, field, limits, onInput, onSubmit, onCancel, testId }: RenameFieldProps) {
  const root = useRef<HTMLDivElement>(null);
  const check = field.check;
  const changed = !field.unchanged;
  const moved = movedWords(field.opened, field.current);
  const note = renameNote(kind, field, limits);
  const save = () => {
    if (!check.error) onSubmit();
  };
  useEffect(() => {
    const input = root.current?.querySelector('input');
    input?.focus();
    input?.select();
  }, []);
  return (
    // The keys are read in the capture phase: the Field would otherwise end the edit on Enter and Escape and the key would never reach the form.
    <div
      ref={root}
      className="flex min-w-0 flex-col gap-1"
      data-testid={`${testId}-form`}
      onKeyDownCapture={(e) => {
        if (e.nativeEvent.isComposing) return;
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
      <div className="flex flex-wrap items-center gap-2">
        <div className="min-w-40 flex-1">
          <Field label={`New name of ${subject}`} size="dense" value={field.text} onInput={onInput} invalid={changed && !!check.error} testId={testId} />
        </div>
        <Button variant="primary" size="sm" aria-label={`Save the new name of ${subject}`} disabled={!!check.error} onClick={save} data-testid={`${testId}-save`}>
          Save
        </Button>
        <Button size="sm" aria-label={`Cancel renaming ${subject}`} onClick={onCancel} data-testid={`${testId}-cancel`}>
          Cancel
        </Button>
      </div>
      <p className={`m-0 text-caption whitespace-normal ${NOTE_CLASS[note.tone]}`} data-testid={`${testId}-note`} data-tone={note.tone}>
        {note.text}
      </p>
      {/* Always on the page, so the words are announced when they are put in. */}
      <p role="status" className="m-0 text-caption whitespace-normal text-text-strong" data-testid={`${testId}-moved`}>
        {moved ?? ''}
      </p>
    </div>
  );
}
