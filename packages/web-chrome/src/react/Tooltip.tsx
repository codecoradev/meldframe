// React copy of Tooltip.svelte and TooltipProvider.svelte, on Base UI. A tooltip on hover and on keyboard focus, never
// a standing element. The trigger is an element, so that the control itself (a button) is the trigger and keeps its own
// name; the tooltip repeats it and adds the shortcut. One TooltipProvider above the controls (the Layout, the matrix
// page) gives the neighbours their shared delay: the first tooltip waits 400 ms, its neighbours then open at once.
import { Tooltip as Base } from '@base-ui/react/tooltip';
import type { ReactElement, ReactNode } from 'react';

/** The wait before the first tooltip of a group opens, and how long after one closed its neighbour still opens at once. */
export const TOOLTIP_DELAY_MS = 400;
export const TOOLTIP_SKIP_MS = 300;

export function TooltipProvider({ children }: { children: ReactNode }) {
  return (
    <Base.Provider delay={TOOLTIP_DELAY_MS} timeout={TOOLTIP_SKIP_MS}>
      {children}
    </Base.Provider>
  );
}

export interface TooltipProps {
  label: string;
  shortcut?: string;
  side?: 'top' | 'bottom' | 'left' | 'right';
  /** The control that shows the tooltip: it gets the hover and focus handlers and keeps its own props. */
  trigger: ReactElement<Record<string, unknown>>;
}

export function Tooltip({ label, shortcut, side = 'bottom', trigger }: TooltipProps) {
  return (
    <Base.Root>
      <Base.Trigger render={trigger} />
      <Base.Portal>
        <Base.Positioner side={side} sideOffset={8} className="z-50">
          <Base.Popup role="tooltip" className="flex items-center gap-2 rounded-md border border-menu-border bg-surface-raised px-2 py-1 text-small text-text shadow-popover">
            {label}
            {shortcut ? <span className="font-mono text-text-muted">{shortcut}</span> : null}
          </Base.Popup>
        </Base.Positioner>
      </Base.Portal>
    </Base.Root>
  );
}
