// React copy of VariablesPanel.svelte. The Variables section of the inspector, as the Variables, ModeSwitch and Inspector boards
// draw it: a search, the collections with their mode switch, and the variables listed by type with a swatch, the name and the
// value of the mode on view in the mono face (an alias as a chip with the name it points at). It draws the rows the app layer
// gives it (order, headings, checks) and reports input as actions (see ../variables.ts).
//
// The three dialogs (the manage view, Import variables, Export variables) are not in this copy: Import and Export variables are Svelte until
// S10, and a Svelte dialog cannot be drawn inside a React root; the manage view has a React copy since S9 (`/?react=manage`) but it is a root of
// its own too. The switch file that hosts this panel (apps/web/src/react-pilot/VariablesPanelSwitch.svelte) draws them beside it and owns
// whether the manage view and Import variables are open; this panel only has the buttons, and tells the host when one is pressed
// (`onManage`, `onImport`).
// Export variables is an action, as in the Svelte copy. A text with a value in it is one string (a template), not two text nodes.
import type { Action, VariableRow, VariablesPanel as Model } from '@meldframe/protocol';
import { Button } from '@meldframe/web-chrome/react/Button';
import { Field } from '@meldframe/web-chrome/react/Field';
import { Segmented } from '@meldframe/web-chrome/react/Segmented';
import { TYPE_LABEL } from '@meldframe/web-document/labels';
import { nameOf } from '@meldframe/web-document/names';
import { Fragment } from 'react';
import { FIELD_LABEL, beforeManage, bindAction, display, listedCount, modeAction, modeName, shownValue, swatch, truncated, valueAction } from '../variables';

export interface VariablesPanelProps {
  panel: Model;
  /** The id of the one selected node, which the bind buttons act on; none for no selection or several. */
  target: string | null;
  dispatch: (a: Action) => void;
  /** The Manage button was pressed (the host opens the manage view, after the search has been cleared here). */
  onManage: () => void;
  /** The Import variables button was pressed (the host opens its dialog). */
  onImport: () => void;
}

export function VariablesPanel({ panel, target, dispatch, onManage, onImport }: VariablesPanelProps) {
  const commit = (row: VariableRow, mode: string) => (raw: string) => dispatch(valueAction(row, mode, raw));
  const manage = () => {
    // The manage view draws the rows of the panel, so it starts with the search cleared (the counts of a removal are the app layer's, of the whole file).
    const clear = beforeManage(panel);
    if (clear) dispatch(clear);
    onManage();
  };
  const shown = panel.collections.filter((c) => c.modes.length > 1);
  return (
    <div className="flex flex-col gap-2 text-small text-text" data-testid="variables">
      <Field label="Search variables" prefix="Search" size="dense" value={panel.query} onInput={(q) => dispatch({ type: 'setVariablesQuery', query: q })} testId="var-search" />
      <div className="flex items-center justify-between gap-2">
        <Button size="sm" onClick={manage} data-testid="var-manage">
          Manage
        </Button>
        <p className="m-0 text-right text-caption text-text-muted" data-testid="var-count">{`${panel.matched} of ${panel.total}`}</p>
      </div>
      <div className="flex flex-wrap items-center gap-2">
        <Button size="sm" onClick={onImport} data-testid="var-import">
          Import variables
        </Button>
        <Button size="sm" onClick={() => dispatch({ type: 'openVariablesExport' })} data-testid="var-export">
          Export variables
        </Button>
      </div>
      {shown.map((c) => (
        <div key={c.id} className="flex flex-col gap-1">
          <span className="truncate text-caption font-semibold text-text-muted" title={nameOf(c.name)}>
            {nameOf(c.name)}
          </span>
          <Segmented label={`Mode of ${nameOf(c.name)}`} options={c.modes.map((m) => ({ value: m.id, label: nameOf(m.name) }))} value={c.active} onChange={(m) => dispatch(modeAction(c, m))} data-testid="var-modes" data-id={c.id} />
        </div>
      ))}
      {shown.length > 0 ? <p className="m-0 text-caption text-text-muted">Switch mode to edit its values.</p> : null}
      <div className="max-h-(--mf-size-variables-list) overflow-y-auto" data-testid="var-list">
        {panel.collections.map((c) => (
          <section key={c.id} className="flex flex-col gap-1" aria-label={nameOf(c.name)} data-testid="var-collection" data-id={c.id}>
            {panel.collections.length > 1 ? (
              <h3 className="m-0 truncate pt-2 text-small font-semibold text-text-strong" title={nameOf(c.name)}>
                {nameOf(c.name)}
              </h3>
            ) : null}
            <ul className="m-0 flex list-none flex-col gap-px p-0">
              {c.variables.map((v) => {
                const val = shownValue(c, v);
                const colour = val ? swatch(v, val.mode) : null;
                const boundNow = v.binds.some((b) => b.bound);
                return (
                  <Fragment key={v.id}>
                    {v.heading ? (
                      <li className="px-1 pt-2 pb-1">
                        <h4 className="m-0 text-small font-normal text-text-muted" data-testid="var-heading">
                          {TYPE_LABEL[v.heading.type]} <span className="text-caption">{v.heading.count}</span>
                        </h4>
                      </li>
                    ) : null}
                    <li className="group flex items-center gap-2" data-testid="var-row" data-id={v.id}>
                      <div className="relative min-w-0 flex-1">
                        <span
                          className={`block truncate ${boundNow ? 'font-medium text-chip-text' : 'text-text-secondary'} ${target && v.binds.length ? 'group-hover:invisible group-focus-within:invisible' : ''}`}
                          title={`${nameOf(v.name)} · ${v.usedBy > 0 ? `used ${v.usedBy}` : 'unused'}`}
                        >
                          {nameOf(v.name)}
                        </span>
                        {target && v.binds.length ? (
                          <div className="absolute inset-y-0 left-0 flex items-center gap-1 opacity-0 group-hover:opacity-100 group-focus-within:opacity-100" role="group" aria-label={`Bind ${nameOf(v.name)} to the selection`}>
                            {v.binds.map((b) => (
                              <Button key={b.field} size="sm" pressed={b.bound} onClick={() => dispatch(bindAction(target, v, b.field, b.bound))} data-testid="var-bind" data-field={b.field}>
                                {FIELD_LABEL[b.field]}
                              </Button>
                            ))}
                          </div>
                        ) : null}
                      </div>
                      {val ? (
                        <div className="w-28 shrink-0" title={display(val)}>
                          <Field
                            label={`${nameOf(v.name)} in ${modeName(c, val.mode)}`}
                            size="dense"
                            mono
                            bound={!!val.alias}
                            value={display(val)}
                            onCommit={commit(v, val.mode)}
                            testId="var-value"
                            lead={colour ? <i className="block size-4 shrink-0 rounded-sm border border-border" style={{ background: colour }} data-testid="var-swatch"></i> : null}
                          />
                        </div>
                      ) : null}
                    </li>
                  </Fragment>
                );
              })}
            </ul>
            {c.matched > c.variables.length ? (
              <p className="m-0 text-caption text-text-muted" data-testid="var-more">{`${c.matched - c.variables.length} more in ${nameOf(c.name)}`}</p>
            ) : null}
          </section>
        ))}
      </div>
      {panel.total === 0 ? (
        <p className="m-0 text-text-muted">This file has no variables yet. An agent can import variables or add them.</p>
      ) : panel.matched === 0 ? (
        <p className="m-0 text-text-muted">No variable matches the search.</p>
      ) : null}
      {truncated(panel) ? (
        <p className="m-0 text-caption text-text-muted" data-testid="var-truncated">{`Showing ${listedCount(panel)} of ${panel.matched}. Narrow the search to see the rest.`}</p>
      ) : null}
    </div>
  );
}
