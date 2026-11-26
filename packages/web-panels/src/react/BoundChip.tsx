// The chip of a field that is bound to a variable, drawn by the panel so every bound field looks alike (the Svelte panel passes it
// to the Fill section as a snippet): the name of the variable, the value it gives, and a button that detaches it. A binding whose
// variable is gone is drawn as a red named chip that says so.
import type { BindField, FieldBinding } from '@meldframe/protocol';
import { IconButton } from '@meldframe/web-chrome/react/IconButton';
import { nameOf } from '@meldframe/web-document/names';

export interface BoundChipProps {
  /** The caption above the chip; the Fill section has none. */
  label: string;
  field: BindField;
  binding: FieldBinding;
  /** The value the variable gives, as the field would have shown it. */
  value: string;
  onDetach: () => void;
}

export function BoundChip({ label, field, binding: b, value, onDetach }: BoundChipProps) {
  // The name is the app layer's, made safe once; a name that is nothing is worded as everywhere else.
  const name = nameOf(b.name);
  return (
    <div className="flex flex-col gap-2" data-testid={`prop-${field}-bound`}>
      {label ? <span className="text-text-muted">{label}</span> : null}
      <div className={`flex h-(--mf-size-control-lg) items-center gap-2 rounded-md px-3 ${b.missing ? 'bg-danger-bg text-danger-text' : 'bg-chip-bg text-chip-text'}`}>
        {field === 'fill' ? <i className="block size-4 shrink-0 rounded-sm border border-border" style={{ background: value }}></i> : null}
        <span className="min-w-0 flex-1 truncate font-medium" title={b.missing ? `${name} (variable removed)` : name}>
          {`${name}${b.missing ? ' (removed)' : ''}`}
        </span>
        <span className="font-mono text-caption" data-testid={field === 'fill' ? 'prop-fill' : `prop-${field}`}>
          {value}
        </span>
        <IconButton label={`Detach ${name}`} icon="close" size={24} onClick={onDetach} data-testid={`prop-${field}-detach`} />
      </div>
    </div>
  );
}
