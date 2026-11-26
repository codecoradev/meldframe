// React copy of CollectionDetail.svelte. One collection of the manage view: its name and what a removal of it touches, its modes (the
// first is the default one and stays), the form that adds a mode, the form that adds a variable, and the variables as a table with
// their type and how many times each is used. Every name is the file's text, drawn as text. A removal does not happen here: it asks
// the parent, which shows the question. A rename is the same: the Rename button asks the parent, and the field that takes the place of
// the name is drawn for the target the parent holds (`rename`). It holds only the type and the source of the values the next
// additions will take. A text with a value in it is one string (a template), not two text nodes.
import type { CollectionRow, ManageDrafts, ManageLimits, ManageTarget, RenameField as RenameState, VarType, VariableEntity } from '@meldframe/protocol';
import { ring } from '@meldframe/web-chrome/focus';
import { Button } from '@meldframe/web-chrome/react/Button';
import { Segmented } from '@meldframe/web-chrome/react/Segmented';
import { TYPE_LABEL } from '@meldframe/web-document/labels';
import { nameOf } from '@meldframe/web-document/names';
import { useState } from 'react';
import { type Summary, count } from '../manage';
import { isEditing } from '../rename';
import { NameForm } from './NameForm';
import { RenameField } from './RenameField';

export interface CollectionDetailProps {
  col: CollectionRow;
  summary: Summary;
  onAsk: (t: ManageTarget) => void;
  /** The field a name is being edited in (`AppState.manage.rename`), or none. */
  rename: RenameState | null;
  drafts: ManageDrafts;
  limits: ManageLimits;
  onRename: (t: ManageTarget) => void;
  onRenameInput: (text: string) => void;
  onRenameSubmit: () => void;
  onRenameCancel: () => void;
  onDraft: (form: VariableEntity, text: string) => void;
  onAddMode: (name: string, copyFrom: string) => void;
  onAddVariable: (name: string, kind: VarType) => void;
}

const TYPES = (['color', 'number'] as const).map((t) => ({ value: t, label: TYPE_LABEL[t] }));
const select = `${ring} h-(--mf-size-control-md) max-w-40 rounded-md border border-divider bg-field px-2 text-small text-text hov:border-border-strong`;

export function CollectionDetail({ col, summary, onAsk, rename, drafts, limits, onRename, onRenameInput, onRenameSubmit, onRenameCancel, onDraft, onAddMode, onAddVariable }: CollectionDetailProps) {
  const [kind, setKind] = useState<VarType>('color');
  const [from, setFrom] = useState('');
  // The mode the next one copies: the default (first) unless another was picked and is still there.
  const copy = col.modes.some((m) => m.id === from) ? from : (col.modes[0]?.id ?? '');
  const name = nameOf(col.name);
  const cut = col.matched > col.variables.length;
  /** Whether the field of a rename is open for this thing. */
  const editing = (k: ManageTarget['kind'], id: string) => isEditing(rename, col.id, k, id);
  const renameProps = { limits, onInput: onRenameInput, onSubmit: onRenameSubmit, onCancel: onRenameCancel };
  return (
    <div className="flex flex-col gap-4 p-4" data-testid="manage-detail-body" data-id={col.id}>
      <header className="flex items-start justify-between gap-3">
        <div className="min-w-0 flex-1">
          {rename && editing('collection', col.id) ? (
            <RenameField kind="collection" subject={`collection ${name}`} testId="manage-renaming-collection" {...renameProps} field={rename} />
          ) : (
            <h3 tabIndex={-1} className="m-0 truncate text-title font-semibold text-text-strong outline-none" title={name} data-testid="manage-title">
              <bdi>{name}</bdi>
            </h3>
          )}
          <p className="m-0 text-caption text-text-muted" data-testid="manage-summary">{`${count(summary.variables, 'variable', 'variables')} · ${summary.complete ? '' : 'at least '}${count(summary.uses, 'use', 'uses')} · ${count(summary.modes, 'mode', 'modes')}`}</p>
        </div>
        <div className="flex shrink-0 gap-2">
          {!editing('collection', col.id) ? (
            <Button size="sm" variant="secondary" aria-label={`Rename collection ${name}`} onClick={() => onRename({ kind: 'collection', id: col.id })} data-testid="manage-rename-collection" data-rename={`collection:${col.id}`}>
              Rename
            </Button>
          ) : null}
          <Button size="sm" variant="secondary" aria-label={`Remove collection ${name}`} onClick={() => onAsk({ kind: 'collection', id: col.id })} data-testid="manage-remove-collection" data-remove={`collection:${col.id}`}>
            Remove collection
          </Button>
        </div>
      </header>

      <section className="flex flex-col gap-2" aria-label={`Modes of ${name}`}>
        <h4 className="m-0 text-body font-semibold text-text-strong">Modes</h4>
        <ul className="m-0 flex list-none flex-wrap gap-2 p-0">
          {col.modes.map((m, i) =>
            rename && editing('mode', m.id) ? (
              <li key={m.id} tabIndex={-1} className={`${ring} w-full rounded-md border border-divider bg-field p-2 text-small text-text`} data-testid="manage-mode" data-id={m.id} data-entity-id={m.id}>
                <RenameField kind="mode" subject={`mode ${nameOf(m.name)}`} testId="manage-renaming-mode" {...renameProps} field={rename} />
              </li>
            ) : (
              <li key={m.id} tabIndex={-1} className={`${ring} flex items-center gap-2 rounded-md border border-divider bg-field py-1 pr-1 pl-3 text-small text-text`} data-testid="manage-mode" data-id={m.id} data-entity-id={m.id}>
                <span className="max-w-48 truncate" title={nameOf(m.name)}>
                  <bdi>{nameOf(m.name)}</bdi>
                </span>
                {i === 0 ? (
                  <span className="text-caption text-text-muted" title="The first mode is the default one and stays">
                    default
                  </span>
                ) : null}
                <Button size="sm" aria-label={`Rename mode ${nameOf(m.name)}`} onClick={() => onRename({ kind: 'mode', collection: col.id, id: m.id })} data-testid="manage-rename-mode" data-rename={`mode:${m.id}`}>
                  Rename
                </Button>
                {i > 0 ? (
                  <Button size="sm" aria-label={`Remove mode ${nameOf(m.name)}`} onClick={() => onAsk({ kind: 'mode', collection: col.id, id: m.id })} data-testid="manage-remove-mode" data-remove={`mode:${m.id}`}>
                    Remove
                  </Button>
                ) : null}
              </li>
            ),
          )}
        </ul>
        <NameForm
          kind="mode"
          label="New mode name"
          submit="Add mode"
          placeholder="dark"
          draft={drafts.mode}
          limits={limits}
          onInput={(t) => onDraft('mode', t)}
          onSubmit={(n) => onAddMode(n, copy)}
          testId="manage-new-mode"
          extra={
            <label className="flex items-center gap-2 text-small text-text-muted">
              <span aria-hidden="true">Copy from</span>
              <select aria-label="Copy the values of the new mode from" className={select} value={copy} onChange={(e) => setFrom(e.currentTarget.value)} data-testid="manage-copy-from">
                {col.modes.map((m) => (
                  <option key={m.id} value={m.id}>
                    {nameOf(m.name)}
                  </option>
                ))}
              </select>
            </label>
          }
        />
      </section>

      <section className="flex flex-col gap-2" aria-label={`Variables of ${name}`}>
        <h4 className="m-0 text-body font-semibold text-text-strong">Variables</h4>
        <NameForm
          kind="variable"
          label="New variable name"
          submit="Add variable"
          placeholder="color/brand-500"
          draft={drafts.variable}
          limits={limits}
          onInput={(t) => onDraft('variable', t)}
          onSubmit={(n) => onAddVariable(n, kind)}
          testId="manage-new-variable"
          extra={
            <div className="w-40">
              <Segmented label="Type of the new variable" options={TYPES} value={kind} onChange={(v) => setKind(v as VarType)} data-testid="manage-new-type" />
            </div>
          }
        />
        {col.variables.length === 0 ? (
          <p className="m-0 text-small text-text-muted" data-testid="manage-no-variables">
            {col.matched === 0 ? 'This collection has no variables yet. Add one above.' : 'No variable of this collection is listed.'}
          </p>
        ) : (
          <>
            <table className="w-full border-collapse text-small" data-testid="manage-vars">
              <caption className="sr-only">{`Variables of ${name}, with their type and how many times each is used`}</caption>
              <thead>
                <tr className="border-b border-hairline text-left text-caption text-text-muted">
                  <th scope="col" className="w-full py-1 pr-2 font-normal">
                    Name
                  </th>
                  <th scope="col" className="px-2 py-1 font-normal">
                    Type
                  </th>
                  <th scope="col" className="px-2 py-1 text-right font-normal">
                    Uses
                  </th>
                  <th scope="col" className="py-1 pl-2 text-right font-normal">
                    <span className="sr-only">Actions</span>
                  </th>
                </tr>
              </thead>
              <tbody>
                {col.variables.map((v) => (
                  <tr key={v.id} tabIndex={-1} className={`${ring} border-b border-hairline`} data-testid="manage-var" data-id={v.id} data-entity-id={v.id}>
                    {rename && editing('variable', v.id) ? (
                      <th scope="row" className="max-w-0 py-1 pr-2 text-left font-normal text-text">
                        <RenameField kind="variable" subject={nameOf(v.name)} testId="manage-renaming-variable" {...renameProps} field={rename} />
                      </th>
                    ) : (
                      <th scope="row" className="max-w-0 truncate py-1 pr-2 text-left font-normal text-text" title={nameOf(v.name)}>
                        <bdi>{nameOf(v.name)}</bdi>
                      </th>
                    )}
                    <td className="px-2 py-1 text-text-secondary" data-testid="manage-var-type">
                      {TYPE_LABEL[v.type]}
                    </td>
                    <td className={`px-2 py-1 text-right font-mono ${v.usedBy === 0 ? 'text-text-muted' : 'text-text'}`} data-testid="manage-var-uses" title={v.usedBy === 0 ? 'Nothing uses it' : count(v.usedBy, 'use', 'uses')}>
                      {v.usedBy}
                    </td>
                    <td className="py-1 pl-2 text-right">
                      <div className="flex justify-end gap-2">
                        {!editing('variable', v.id) ? (
                          <Button size="sm" aria-label={`Rename ${nameOf(v.name)}`} onClick={() => onRename({ kind: 'variable', id: v.id })} data-testid="manage-rename-variable" data-rename={`variable:${v.id}`}>
                            Rename
                          </Button>
                        ) : null}
                        <Button size="sm" aria-label={`Remove ${nameOf(v.name)}`} onClick={() => onAsk({ kind: 'variable', id: v.id })} data-testid="manage-remove-variable" data-remove={`variable:${v.id}`}>
                          Remove
                        </Button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
            {cut ? (
              <p className="m-0 text-caption text-text-muted" data-testid="manage-cut">{`Showing ${col.variables.length} of ${col.matched} variables.`}</p>
            ) : null}
          </>
        )}
      </section>
    </div>
  );
}
