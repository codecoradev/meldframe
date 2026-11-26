// React copy of LayersPanel.svelte. The layers panel: the tree of the file, the selection, and who changed each node last. It
// renders the rows it is given and reports clicks; it holds no state of its own. What it draws from a row apart from the markup
// (the icon of a kind, the actor's name and dot) is `layers.ts`, which the Svelte copy calls too.
import { Icon } from '@meldframe/web-chrome/react/Icon';
import type { LayerRow } from '@meldframe/protocol';
import { nameOf } from '@meldframe/web-document/names';
import { KIND_ICON, changedBy } from '../layers';

export interface LayersPanelProps {
  rows: readonly LayerRow[];
  /** The actor of this page: its own changes carry no label. */
  self: string;
  onSelect: (id: string, additive: boolean) => void;
  onToggle: (id: string) => void;
}

export function LayersPanel({ rows, self, onSelect, onToggle }: LayersPanelProps) {
  return (
    <div className="flex flex-col px-3 pb-3 text-body text-text-secondary" data-testid="layers">
      <h3 id="layers-title" className="m-0 py-2 text-body font-semibold text-text-strong">
        Layers
      </h3>
      <ul className="m-0 flex list-none flex-col gap-px p-0" aria-labelledby="layers-title">
        {rows.map((r) => {
          const who = changedBy(r, self);
          return (
            <li
              key={r.id}
              className={`group flex h-(--mf-size-control-md) items-center gap-2 rounded-sm pr-2 ${r.selected ? 'bg-selection-bg text-selection-text font-medium' : 'hov:bg-hairline'}`}
              style={{ paddingLeft: `calc(var(--mf-space-2) + ${r.depth} * var(--mf-space-5))` }}
              data-testid="layer"
              data-id={r.id}
              data-selected={r.selected}
            >
              {r.hasChildren ? (
                <button
                  type="button"
                  className="flex h-full min-w-(--mf-size-control-sm) shrink-0 items-center justify-center rounded-xs text-text-muted"
                  aria-label={`${r.collapsed ? 'Expand' : 'Collapse'} ${nameOf(r.name)}`}
                  aria-expanded={!r.collapsed}
                  onClick={() => onToggle(r.id)}
                >
                  <Icon name={r.collapsed ? 'chevron-right' : 'chevron-down'} />
                </button>
              ) : (
                <span className="min-w-(--mf-size-control-sm) shrink-0"></span>
              )}
              <button
                type="button"
                className="flex h-full min-h-(--mf-size-control-sm) min-w-0 flex-1 items-center gap-2 truncate text-left"
                aria-current={r.selected ? 'true' : undefined}
                onClick={(e) => onSelect(r.id, e.shiftKey)}
              >
                <Icon name={KIND_ICON[r.kind]} />
                <span className="truncate">{nameOf(r.name)}</span>
              </button>
              {who ? (
                <span className="flex min-w-0 max-w-24 shrink-0 items-center gap-1" title={`Last changed by ${who.name}`} data-testid="actor" data-tone={who.tone}>
                  <span className={`truncate text-caption ${r.selected ? 'text-selection-text' : 'text-text-muted'}`}>{who.name}</span>
                  <span className={`size-2 shrink-0 rounded-pill ${who.dot}`}></span>
                </span>
              ) : null}
            </li>
          );
        })}
        {rows.length === 0 ? <li className="py-2 text-text-muted">Nothing here yet.</li> : null}
      </ul>
    </div>
  );
}
