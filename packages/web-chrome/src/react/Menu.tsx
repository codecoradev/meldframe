// React copy of Menu.svelte, on Base UI. A menu behind an icon button: rows with a label, an optional shortcut, and a
// disabled state. It holds no state: a row calls `onSelect`. The trigger is an IconButton, so the label is its name and
// tooltip. With `text` the trigger is a plain text button instead (the zoom readout): the text is what it shows and
// the label its name, which repeats the text.
import { Menu as Base } from '@base-ui/react/menu';
import { useRef } from 'react';
import { type ControlSize, rememberFocus, ring } from '../focus';
import type { IconName } from '../icons';
import { IconButton } from './IconButton';

export interface MenuItem {
  id: string;
  label: string;
  onSelect: () => void;
  disabled?: boolean;
  shortcut?: string;
  /** `data-testid` of the row. */
  testId?: string;
  /** The row opens a dialog, which takes the focus: the menu does not give it back to its button. */
  opensDialog?: boolean;
}

export interface MenuProps {
  label: string;
  icon?: IconName;
  /** Shows this text as the trigger instead of the icon. */
  text?: string;
  items: MenuItem[];
  size?: ControlSize;
  testId?: string;
  side?: 'top' | 'bottom' | 'left' | 'right';
  align?: 'start' | 'center' | 'end';
}

const row = 'flex h-(--mf-size-control-lg) cursor-default items-center justify-between gap-4 rounded-sm px-3 outline-none';

export function Menu({ label, icon = 'menu', text, items, size = 32, testId, side = 'bottom', align = 'start' }: MenuProps) {
  const keepFocus = useRef(false);
  const trigger = useRef<HTMLButtonElement>(null);
  return (
    <Base.Root>
      <Base.Trigger
        render={
          text ? (
            <button ref={trigger} type="button" aria-label={label} data-testid={testId} className={`${ring} h-(--mf-size-control-lg) rounded-md border border-transparent px-2 font-mono text-small text-text-muted hov:bg-control hov:text-text-strong act:bg-control-selected`} />
          ) : (
            <IconButton ref={trigger} label={label} icon={icon} size={size} data-testid={testId} />
          )
        }
      >
        {text}
      </Base.Trigger>
      <Base.Portal>
        <Base.Positioner side={side} sideOffset={8} align={align} className="z-50">
          <Base.Popup
            data-owns-keys=""
            finalFocus={() => {
              // A row that opened a dialog leaves the focus where the dialog took it; any other close gives it back to the button.
              const keep = keepFocus.current;
              keepFocus.current = false;
              return !keep;
            }}
            className="min-w-48 rounded-lg border border-menu-border bg-surface-raised p-1 text-small text-text shadow-popover"
          >
            {items.map((it) => {
              const content = (
                <>
                  {it.label}
                  {it.shortcut ? <span className="font-mono text-caption text-text-muted">{it.shortcut}</span> : null}
                </>
              );
              // Base UI keeps a disabled row in the arrow-key order (no option turns it off), and the Svelte menu skips it. A
              // disabled row is therefore drawn by hand, as a menu item that is not one of the list: no arrow, hover, typeahead
              // or click reaches it, and a reader still finds it named and disabled.
              return it.disabled ? (
                <div key={it.id} role="menuitem" aria-disabled="true" data-disabled="" data-testid={it.testId} className={`${row} opacity-(--mf-opacity-disabled)`}>
                  {content}
                </div>
              ) : (
                <Base.Item
                  key={it.id}
                  label={it.label}
                  onClick={() => {
                    keepFocus.current = !!it.opensDialog;
                    if (keepFocus.current) rememberFocus(trigger.current);
                    it.onSelect();
                  }}
                  data-testid={it.testId}
                  className={`${row} data-highlighted:bg-control`}
                >
                  {content}
                </Base.Item>
              );
            })}
          </Base.Popup>
        </Base.Positioner>
      </Base.Portal>
    </Base.Root>
  );
}
