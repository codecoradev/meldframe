// React copy of PickerList.svelte. What is inside the bind picker of a field: the search, the variables that fit the field (a
// bound one is pressed), and the notes under them. The app layer says which type fits (`picker.type`) and lists the rows; this
// draws them. It holds no state of its own. A text with a value in it is one string (a template), not two text nodes.
import type { Action, Picker } from '@meldframe/protocol';
import { Field } from '@meldframe/web-chrome/react/Field';
import { TYPE_LABEL } from '@meldframe/web-document/labels';
import { nameOf } from '@meldframe/web-document/names';
import { pickAction, pickerQueryAction } from '@meldframe/web-document/picker';
import { FIELD_LABEL } from '../variables';

export interface PickerListProps {
  picker: Picker;
  dispatch: (a: Action) => void;
}

export function PickerList({ picker, dispatch }: PickerListProps) {
  const type = TYPE_LABEL[picker.type].toLowerCase();
  const label = FIELD_LABEL[picker.field];
  return (
    <>
      <div className="border-b border-menu-border p-2">
        <Field label="Search variables" prefix="Search" size="dense" value={picker.query} onInput={(q) => dispatch(pickerQueryAction(q))} testId="picker-search" />
      </div>
      <ul className="m-0 flex max-h-[calc(var(--mf-size-control-lg)*6)] list-none flex-col gap-px overflow-y-auto p-2" aria-label={`Variables for ${label}`}>
        {picker.rows.map((row) => (
          <li key={row.id}>
            <button
              type="button"
              aria-pressed={row.bound}
              onClick={() => dispatch(pickAction(picker, row))}
              data-testid="picker-row"
              data-id={row.id}
              className={`flex h-(--mf-size-control-lg) w-full items-center gap-2 rounded-sm px-2 text-left hov:bg-control ${row.bound ? 'bg-chip-bg text-chip-text' : ''}`}
            >
              {row.type === 'color' && row.resolved ? <i className="block size-4 shrink-0 rounded-sm border border-border" style={{ background: row.resolved }}></i> : null}
              <span className="min-w-0 flex-1 truncate" title={`${nameOf(row.name)} · ${nameOf(row.collection)}`}>
                {nameOf(row.name)}
              </span>
              <span className="font-mono text-caption text-text-muted">{row.resolved}</span>
            </button>
          </li>
        ))}
      </ul>
      {picker.rows.length === 0 ? (
        <p className="m-0 px-4 pb-2 text-text-muted" data-testid="picker-empty">
          {picker.total === 0 ? `This file has no ${type} variables.` : 'No variable matches the search.'}
        </p>
      ) : null}
      <p className="m-0 border-t border-menu-border px-4 py-2 text-caption text-text-muted">{`Only ${type} variables are shown for ${label}.`}</p>
    </>
  );
}
