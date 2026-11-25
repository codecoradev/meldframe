// Development only, part of the React pilot (docs/react-web-plan.md, section 7): the React side of the overlay fixture at
// /?overlays=react. It draws the React IconButton and Menu, with the tooltip they bring, and the same test ids as
// OverlayFixtureSvelte.svelte (/?overlays=svelte), so that packages/web-chrome/scripts/overlays-e2e.mjs asks both the same
// questions. Not in production: main.ts loads it in development only.
import { useEffect, useRef, useState } from 'react';
import { IconButton } from '@meldframe/web-chrome/react/IconButton';
import { Menu } from '@meldframe/web-chrome/react/Menu';
import { TooltipProvider } from '@meldframe/web-chrome/react/Tooltip';
import { fixtureItems } from './fixture';

export function OverlayFixture() {
  const [log, setLog] = useState<string[]>([]);
  const [dialog, setDialog] = useState(false);
  // The rows are made once: a menu row keeps its handler for as long as the page lives.
  const [items] = useState(() =>
    fixtureItems((id) => {
      setLog((l) => [...l, id]);
      if (id === 'dialog') setDialog(true);
    }),
  );
  // The stand-in for a dialog: it takes the focus when it opens, as the Export dialog does.
  const field = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (dialog) field.current?.focus();
  }, [dialog]);

  return (
    <TooltipProvider>
      <main className="flex flex-col items-start gap-4 p-8 font-sans text-body text-text">
        <div className="mt-64 flex gap-2">
          <IconButton label="Layers panel" icon="panel-left" shortcut="Alt+1" data-testid="fx-icon" />
          <IconButton label="Inspector panel" icon="panel-right" shortcut="Alt+2" data-testid="fx-icon-2" />
          <Menu label="Fixture menu" icon="menu" items={items} testId="fx-menu" />
          <Menu label="Zoom 100%" text="100%" items={items} testId="fx-text-menu" side="top" align="end" />
        </div>
        <button type="button" data-testid="fx-outside" className="h-(--mf-size-control-lg) rounded-md bg-control px-4 text-small">
          Outside
        </button>
        <output data-testid="fx-log">{log.join(',')}</output>
        {dialog ? <input ref={field} data-testid="fx-dialog-input" aria-label="Dialog stand-in" className="h-(--mf-size-control-lg) rounded-md border border-divider bg-field px-3 text-small" /> : null}
      </main>
    </TooltipProvider>
  );
}
