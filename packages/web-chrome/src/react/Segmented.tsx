// React copy of Segmented.svelte. A segmented control: one choice of a few, as a radio group. Each option has a `label`
// (the accessible name; shown unless the option is icon-only). Arrow keys, Home and End move the choice; only the
// chosen option is in the Tab order.
import { type KeyboardEvent, useRef } from 'react';
import { dim, nextIndex, requireLabel, ring } from '../focus';
import type { IconName } from '../icons';
import { Icon } from './Icon';

export interface SegmentedOption {
  value: string;
  label: string;
  icon?: IconName;
}

export interface SegmentedProps {
  label: string;
  options: SegmentedOption[];
  value: string;
  onChange: (value: string) => void;
  disabled?: boolean;
  /** Show the icon alone; the label stays as the name. */
  iconOnly?: boolean;
  [data: `data-${string}`]: string | undefined;
}

export function Segmented({ label, options, value, onChange, disabled = false, iconOnly = false, ...rest }: SegmentedProps) {
  const group = requireLabel(label, 'Segmented');
  const current = Math.max(0, options.findIndex((o) => o.value === value));
  const host = useRef<HTMLDivElement>(null);

  function onKeyDown(e: KeyboardEvent, i: number) {
    const to = nextIndex(e.key, i, options.length);
    if (to === undefined) return;
    e.preventDefault();
    onChange(options[to]!.value);
    host.current?.querySelectorAll<HTMLElement>('[role=radio]')[to]?.focus();
  }

  return (
    <div ref={host} {...rest} role="radiogroup" aria-label={group} aria-disabled={disabled || undefined} className={`${disabled ? dim : ''} flex gap-0.5 rounded-md bg-field p-0.5`}>
      {options.map((o, i) => {
        const on = o.value === value;
        const look = on ? 'border border-border-strong bg-control-selected font-semibold text-text-strong' : 'border border-transparent text-text-muted hov:text-text';
        return (
          <button
            key={o.value}
            type="button"
            role="radio"
            aria-checked={on}
            aria-label={iconOnly ? o.label : undefined}
            disabled={disabled}
            tabIndex={i === current ? 0 : -1}
            onClick={() => onChange(o.value)}
            onKeyDown={(e) => onKeyDown(e, i)}
            className={`${ring} flex h-(--mf-size-control-md) min-w-0 flex-auto items-center justify-center gap-1 rounded-sm ${options.length > 4 ? 'px-1' : 'px-2'} text-small ${look}`}
            title={o.label}
          >
            {o.icon ? <Icon name={o.icon} /> : null}
            {iconOnly ? null : <span className="min-w-0 truncate">{o.label}</span>}
          </button>
        );
      })}
    </div>
  );
}
