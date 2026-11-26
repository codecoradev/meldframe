// React copy of CollectionList.svelte. The collections of the file, one button each (the chosen one is current), and the form
// that adds one. Names are the file's text, drawn as text with `nameOf` standing in for a name that shows as nothing. It holds no
// state of its own. A text with a value in it is one string (a template), not two text nodes.
import type { CollectionRow, ManageLimits, NameDraft } from '@meldframe/protocol';
import { ring } from '@meldframe/web-chrome/focus';
import { nameOf } from '@meldframe/web-document/names';
import { count } from '../manage';
import { NameForm } from './NameForm';

export interface CollectionListProps {
  collections: CollectionRow[];
  /** The id of the collection the detail shows. */
  chosen: string | null;
  onChoose: (id: string) => void;
  /** The text of the form that adds one and its check (`AppState.manage.drafts.collection`). */
  draft: NameDraft;
  limits: ManageLimits;
  onInput: (text: string) => void;
  onAdd: (name: string) => void;
}

export function CollectionList({ collections, chosen, onChoose, draft, limits, onInput, onAdd }: CollectionListProps) {
  return (
    <section className="flex flex-col gap-3 p-4" aria-label="Collections">
      <h3 tabIndex={-1} className="m-0 text-body font-semibold text-text-strong outline-none" data-testid="manage-collections-title">
        Collections
      </h3>
      {collections.length === 0 ? (
        <p className="m-0 text-small text-text-muted" data-testid="manage-no-collections">
          This file has no collections yet. Add one to start.
        </p>
      ) : null}
      <ul className="m-0 flex list-none flex-col gap-px p-0">
        {collections.map((c) => {
          const on = c.id === chosen;
          return (
            <li key={c.id}>
              <button
                type="button"
                aria-current={on ? 'true' : undefined}
                onClick={() => onChoose(c.id)}
                data-testid="manage-collection"
                data-id={c.id}
                data-entity-id={c.id}
                className={`${ring} flex w-full flex-col rounded-md px-2 py-1 text-left hov:bg-control ${on ? 'bg-chip-bg text-chip-text' : 'text-text'}`}
              >
                <span className="truncate text-small font-medium" title={nameOf(c.name)}>
                  <bdi>{nameOf(c.name)}</bdi>
                </span>
                <span className={`text-caption ${on ? '' : 'text-text-muted'}`}>{`${count(c.matched, 'variable', 'variables')} · ${count(c.modes.length, 'mode', 'modes')}`}</span>
              </button>
            </li>
          );
        })}
      </ul>
      <NameForm kind="collection" label="New collection name" submit="Add collection" placeholder="color" draft={draft} limits={limits} onInput={onInput} onSubmit={onAdd} testId="manage-new-collection" />
    </section>
  );
}
