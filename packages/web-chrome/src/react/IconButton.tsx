// React copy of IconButton.svelte. A button that shows an icon only. `label` is required: it becomes the accessible name
// and the tooltip. `pressed` makes it a toggle (aria-pressed) drawn selected: a fill and a 1 px border-strong outline,
// because the fill alone is about 1.3:1 against a card. Sizes 24, 28, 32 and 36 come from the size tokens. A menu
// that wraps it as its trigger hands it props and a ref: they land on the button, after the tooltip's own.
import type { ComponentProps } from 'react';
import { type ControlSize, disabled as off, requireLabel, ring, square } from '../focus';
import type { IconName } from '../icons';
import { Icon } from './Icon';
import { Tooltip } from './Tooltip';

export interface IconButtonProps extends Omit<ComponentProps<'button'>, 'aria-label' | 'children' | 'className' | 'type'> {
  label: string;
  icon: IconName;
  size?: ControlSize;
  shortcut?: string;
  pressed?: boolean;
  [data: `data-${string}`]: string | undefined;
}

export function IconButton({ label, icon, size = 32, shortcut, pressed, disabled = false, ...rest }: IconButtonProps) {
  const name = requireLabel(label, 'IconButton');
  const look = pressed
    ? 'border-border-strong bg-control text-text-strong act:bg-control-selected'
    : 'border-transparent text-text-muted hov:bg-control hov:text-text-strong act:bg-control-selected';
  return (
    <Tooltip
      label={name}
      shortcut={shortcut}
      side="bottom"
      trigger={
        <button {...rest} type="button" aria-label={name} aria-pressed={pressed} disabled={disabled} className={`${square[size]} ${ring} ${off} flex shrink-0 items-center justify-center rounded-md border ${look}`}>
          <Icon name={icon} />
        </button>
      }
    />
  );
}
