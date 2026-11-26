// React copy of Section.svelte. A section of a panel: a title, optional actions on the right, a rule above it. With
// `onToggle` the title is a button that folds the section (aria-expanded); without it the title is plain text. The body
// is the caller's.
import type { ReactNode } from 'react';
import { ring } from '../focus';
import { Icon } from './Icon';

export interface SectionProps {
  title: string;
  /** Folding: `open` and the handler that flips it; omit both for a section that stays open. */
  open?: boolean;
  onToggle?: () => void;
  /** `data-testid` of the folding button. */
  toggleId?: string;
  actions?: ReactNode;
  children: ReactNode;
  [data: `data-${string}`]: string | undefined;
}

export function Section({ title, open = true, onToggle, toggleId, actions, children, ...rest }: SectionProps) {
  return (
    <section {...rest} aria-label={title} className="flex flex-col gap-3 border-t border-hairline px-4 py-4 first:border-t-0">
      <header className="flex items-center justify-between text-body font-semibold text-text-strong">
        {onToggle ? (
          <button type="button" aria-expanded={open} data-testid={toggleId} onClick={onToggle} className={`${ring} -ml-1 flex h-(--mf-size-control-sm) items-center gap-1 rounded-sm px-1`}>
            <span className="text-icon-muted">
              <Icon name={open ? 'chevron-down' : 'chevron-right'} />
            </span>
            {title}
          </button>
        ) : (
          <h3 className="m-0 text-[inherit]">{title}</h3>
        )}
        {actions ? <span className="flex">{actions}</span> : null}
      </header>
      {open ? children : null}
    </section>
  );
}
