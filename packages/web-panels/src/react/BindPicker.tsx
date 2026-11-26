// React copy of BindPicker.svelte, on Base UI's Popover. The bind button of a field and its picker, as the Inspector board
// draws it: the variable button at the end of a field opens a popover with a search and the variables that fit the field (the
// app layer lists them, only while it is open). A row binds the field and the app layer closes the popover (it stays open when
// the bind is refused). What is inside is PickerList. It holds no state of its own: it is open while the app state says so.
import { Popover as Base } from '@base-ui/react/popover';
import type { Action, BindField, Picker, Properties } from '@meldframe/protocol';
import { IconButton } from '@meldframe/web-chrome/react/IconButton';
import { TooltipProvider } from '@meldframe/web-chrome/react/Tooltip';
import { closePickerAction, openPickerAction } from '@meldframe/web-document/picker';
import type { KeyboardEvent } from 'react';
import { FIELD_LABEL } from '../variables';
import { PickerList } from './PickerList';

export interface BindPickerProps {
  properties: Properties;
  field: BindField;
  /** The picker of the app state; this one is open when it is the picker of this field of this node. */
  picker: Picker | null;
  dispatch: (a: Action) => void;
}

// What Tab can reach inside the popover: the search field and the rows.
const TABBABLE = 'a[href],button:not(:disabled),input:not(:disabled),select:not(:disabled),textarea:not(:disabled),[tabindex]:not([tabindex="-1"])';

// Bits UI keeps Tab inside the popover (a trap), and Base UI's non-modal popover does not: Tab from the last row left it and closed it. The
// trap is written here, in the capture phase because the search field stops the keys it owns: Tab from the last stop goes to the first and
// Shift+Tab from the first goes to the last, and nothing else changes (no focus guard, so no extra stop for a reader or a keyboard).
function keepTabInside(e: KeyboardEvent<HTMLElement>) {
  if (e.key !== 'Tab') return;
  const stops = [...e.currentTarget.querySelectorAll<HTMLElement>(TABBABLE)];
  const first = stops[0];
  const last = stops[stops.length - 1];
  if (!first || !last) return;
  const at = document.activeElement;
  if (e.shiftKey && (at === first || at === e.currentTarget)) {
    e.preventDefault();
    last.focus();
  } else if (!e.shiftKey && at === last) {
    e.preventDefault();
    first.focus();
  }
}

export function BindPicker({ properties: p, field, picker, dispatch }: BindPickerProps) {
  const mine = picker && picker.id === p.id && picker.field === field ? picker : null;
  return (
    // An island is a root of its own: the provider the Svelte layout gives the button from above is here.
    <TooltipProvider>
      <Base.Root open={!!mine} onOpenChange={(o) => dispatch(o ? openPickerAction(p, field) : closePickerAction())}>
        <Base.Trigger render={<IconButton label={`Bind ${FIELD_LABEL[field]} to a variable`} icon="variable" size={24} data-testid={`prop-${field}-bind`} />} />
        <Base.Portal>
          <Base.Positioner side="bottom" align="end" sideOffset={8} className="z-50">
            <Base.Popup data-owns-keys="" data-testid="picker" aria-label={`Bind ${FIELD_LABEL[field]} to a variable`} onKeyDownCapture={keepTabInside} className="flex w-72 flex-col overflow-hidden rounded-lg border border-menu-border bg-surface-raised text-small text-text shadow-popover">
              {mine ? <PickerList picker={mine} dispatch={dispatch} /> : null}
            </Base.Popup>
          </Base.Positioner>
        </Base.Portal>
      </Base.Root>
    </TooltipProvider>
  );
}
