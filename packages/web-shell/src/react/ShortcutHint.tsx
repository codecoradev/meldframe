// React copy of ShortcutHint.svelte. The shortcut sheet: a card with every key and gesture the editor answers to today, in
// groups (the list is `shortcuts.ts`, checked against the real handlers), opened by `?` or the help button and closed by
// Escape. It scrolls when the window is short (the card around it sets the room it has). It is not a Base UI dialog: it does
// not hide the page, as the Svelte copy does not. The wrapper that places it is the Layout's.
import { takeFocusBack } from '@meldframe/web-chrome/focus';
import { IconButton } from '@meldframe/web-chrome/react/IconButton';
import { TooltipProvider } from '@meldframe/web-chrome/react/Tooltip';
import { useEffect, useRef } from 'react';
import { onMac } from '../platform';
import { sheetGroups } from '../shortcuts';

export interface ShortcutHintProps {
  onClose: () => void;
  autoLayout?: boolean;
}

export function ShortcutHint({ onClose, autoLayout = true }: ShortcutHintProps) {
  const root = useRef<HTMLDivElement>(null);
  // Focus goes in when the sheet opens and back to where it was when it closes; Escape acts only while the focus is in
  // the sheet, so an Escape meant for a menu or a text edit is not taken by it.
  useEffect(() => {
    // Opened from the palette, the focus goes back to where the palette was opened from, which it remembered for this.
    const before = (takeFocusBack() as HTMLElement | null) ?? (document.activeElement as HTMLElement | null);
    root.current?.querySelector<HTMLElement>('button')?.focus();
    return () => {
      if (before?.isConnected) before.focus();
    };
  }, []);
  const groups = sheetGroups(onMac(), autoLayout);
  return (
    <TooltipProvider>
      <div
        ref={root}
        role="dialog"
        tabIndex={-1}
        aria-label="Keyboard shortcuts"
        // In the capture phase: the tooltip of the Close button, open while the button has the focus, takes the Escape it hears
        // (Base UI stops it at the trigger), and the sheet would need a second one; the Svelte sheet closes on the first.
        onKeyDownCapture={(e) => {
          if (e.key === 'Escape') {
            e.stopPropagation();
            onClose();
          }
        }}
        data-testid="shortcut-hint"
        className="pointer-events-auto flex max-h-full w-(--mf-size-inspector) flex-col gap-1 overflow-y-auto rounded-lg border border-menu-border bg-surface-raised p-3 text-small text-text shadow-popover"
      >
        <div className="flex items-center justify-between font-semibold text-text-strong">
          Shortcuts
          <IconButton label="Close shortcuts" icon="close" size={24} onClick={onClose} />
        </div>
        {groups.map((g) => (
          <section key={g.id} aria-labelledby={`shortcut-group-${g.id}`} className="flex flex-col gap-1 pt-2">
            <h2 id={`shortcut-group-${g.id}`} className="text-caption font-semibold text-text-muted">
              {g.title}
            </h2>
            {g.rows.map((r) => (
              <div key={r.id} className="flex items-center justify-between gap-3">
                <span>{r.name}</span>
                <span className="flex shrink-0 gap-1">
                  {r.keys.map((key) => (
                    <kbd key={key} className="rounded-xs border border-menu-border bg-field px-1 font-mono text-caption">
                      {key}
                    </kbd>
                  ))}
                </span>
              </div>
            ))}
          </section>
        ))}
        <p className="pt-2 text-caption text-text-muted">The view keys work while the canvas has the focus.</p>
      </div>
    </TooltipProvider>
  );
}
