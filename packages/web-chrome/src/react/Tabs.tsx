// React copy of Tabs.svelte. The tab strip of the right panel (Design and Activity): a tab list with one selected tab,
// as the Activity board draws it. Arrow keys, Home and End move the selection; the selected tab alone is in the Tab
// order. `badge` is a count.
import { type KeyboardEvent, useRef } from 'react';
import { nextIndex, requireLabel, ring } from '../focus';

export interface TabItem {
  id: string;
  label: string;
  /** A count; `null` is a count that is not known, drawn as `?` and never as nothing or zero. */
  badge?: number | null;
  /** What the badge counts, for a reader who does not see its colour: read after the number. */
  badgeLabel?: string;
  /** Not available yet: it cannot be selected by click or arrow, and it has no panel (no aria-controls). */
  disabled?: boolean;
}

export interface TabsProps {
  label: string;
  tabs: TabItem[];
  selected: string;
  onSelect: (id: string) => void;
  /** The panel of a tab is `${idPrefix}-panel-${id}`, the tab itself `${idPrefix}-tab-${id}`. */
  idPrefix: string;
  [data: `data-${string}`]: string | undefined;
}

export function Tabs({ label, tabs, selected, onSelect, idPrefix, ...rest }: TabsProps) {
  const name = requireLabel(label, 'Tabs');
  const current = Math.max(0, tabs.findIndex((t) => t.id === selected));
  const host = useRef<HTMLDivElement>(null);

  function onKeyDown(e: KeyboardEvent, i: number) {
    const to = nextIndex(e.key, i, tabs.length, tabs.map((t) => !!t.disabled));
    if (to === undefined) return;
    e.preventDefault();
    onSelect(tabs[to]!.id);
    host.current?.querySelectorAll<HTMLElement>('[role=tab]')[to]?.focus();
  }

  return (
    <div ref={host} {...rest} role="tablist" aria-label={name} className="flex min-w-0 flex-1 gap-1">
      {tabs.map((t, i) => {
        const on = t.id === selected;
        const look = on ? 'bg-hairline font-semibold text-text-strong' : t.disabled ? 'font-medium text-text-muted opacity-(--mf-opacity-disabled)' : 'font-medium text-text-muted hov:text-text';
        return (
          <button
            key={t.id}
            type="button"
            role="tab"
            id={`${idPrefix}-tab-${t.id}`}
            aria-selected={on}
            aria-controls={t.disabled ? undefined : `${idPrefix}-panel-${t.id}`}
            aria-disabled={t.disabled ? 'true' : undefined}
            tabIndex={i === current ? 0 : -1}
            onClick={() => {
              if (!t.disabled) onSelect(t.id);
            }}
            onKeyDown={(e) => onKeyDown(e, i)}
            className={`${ring} flex h-(--mf-size-control-md) items-center gap-2 rounded-md px-3 text-small ${look}`}
          >
            {t.label}
            {t.badge === null ? (
              <span className="rounded-pill bg-accent-strong px-2 text-caption font-bold text-on-accent" data-unknown="true">
                ?
                {t.badgeLabel ? <span className="sr-only">{` ${t.badgeLabel}: unknown`}</span> : null}
              </span>
            ) : t.badge ? (
              <span className="rounded-pill bg-accent-strong px-2 text-caption font-bold text-on-accent">
                {t.badge}
                {t.badgeLabel ? <span className="sr-only">{` ${t.badgeLabel}`}</span> : null}
              </span>
            ) : null}
          </button>
        );
      })}
    </div>
  );
}
