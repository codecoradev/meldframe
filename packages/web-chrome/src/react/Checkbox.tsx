// React copy of Checkbox.svelte. A checkbox with its label, as the Properties board draws it (Clip content): a 16 px box
// that fills with the accent and shows a check. A native checkbox underneath, so the keyboard and the screen reader
// work as they do everywhere.
import { dim, requireLabel, ringWithin } from '../focus';
import { Icon } from './Icon';

export interface CheckboxProps {
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
  [data: `data-${string}`]: string | undefined;
}

export function Checkbox({ label, checked, onChange, disabled = false, ...rest }: CheckboxProps) {
  const name = requireLabel(label, 'Checkbox');
  const box = checked ? 'border-transparent bg-accent-strong text-on-accent' : 'border-border-strong bg-field';
  return (
    <label className={`${disabled ? dim : ''} flex h-(--mf-size-control-md) cursor-pointer items-center gap-2 text-small text-text-secondary`}>
      <span className={`${ringWithin} relative flex size-4 shrink-0 items-center justify-center rounded-sm border ${box}`}>
        <input {...rest} type="checkbox" className="absolute inset-0 m-0 cursor-pointer opacity-0" checked={checked} disabled={disabled} onChange={(e) => onChange(e.currentTarget.checked)} />
        {checked ? <Icon name="check" /> : null}
      </span>
      {name}
    </label>
  );
}
