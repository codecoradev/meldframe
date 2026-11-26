// React copy of PaletteBody.svelte. The inside of the command palette: a search field and the list it narrows
// (`palette.ts` decides what is listed, in what order, which row is highlighted and what the arrows and Enter do; the same
// functions the Svelte copy calls). A combobox in the pattern of the ARIA guide: the focus stays in the field, the highlighted
// row is named by `aria-activedescendant`, a row that cannot run now is listed but disabled. It keeps view state only (the
// text typed and the row the pointer chose); the dialog around it is `Palette.tsx`. A text with a value in it is one string.
import { type KeyboardEvent, type Ref, useEffect, useMemo, useState } from 'react';
import { layoutKey } from '../keys';
import { filterEntries, highlightAt, type PaletteEntry, paletteKey, resultsText } from '../palette';

export interface PaletteBodyProps {
  entries: PaletteEntry[];
  /** The person ran an entry (Enter, or a click on a row that can run). */
  onRun: (e: PaletteEntry) => void;
  /** Ctrl+K again: the browser's own use of it (a search bar) is held back and the palette closes. */
  onClose: () => void;
  /** The search field, for the dialog that gives it the first focus. */
  inputRef?: Ref<HTMLInputElement>;
}

const rowId = (e: PaletteEntry) => `palette-option-${e.id}`;
const row = 'flex h-(--mf-size-control-lg) items-center justify-between gap-4 rounded-sm px-3 text-small text-text';

export function PaletteBody({ entries, onRun, onClose, inputRef }: PaletteBodyProps) {
  const [query, setQuery] = useState('');
  // The row the pointer chose; typing forgets it, and the highlight starts again on the first row that can run.
  const [picked, setPicked] = useState<string | null>(null);
  const shown = useMemo(() => filterEntries(entries, query), [entries, query]);
  const at = highlightAt(shown, picked);
  const current = shown[at];
  const keydown = (ev: KeyboardEvent<HTMLInputElement>) => {
    if (ev.nativeEvent.isComposing) return;
    if (layoutKey(ev) === 'palette') {
      ev.preventDefault();
      ev.stopPropagation();
      onClose();
      return;
    }
    const k = paletteKey(ev.key, at, shown);
    if (!k) return;
    ev.preventDefault();
    if (k.kind === 'run') onRun(shown[k.index]!);
    else setPicked(shown[k.index]!.id);
  };
  // The highlighted row stays in view as the arrows move it through a long list.
  const currentId = current ? rowId(current) : null;
  useEffect(() => {
    if (currentId) document.getElementById(currentId)?.scrollIntoView?.({ block: 'nearest' });
  }, [currentId]);

  return (
    <div className="flex flex-col">
      <input
        ref={inputRef}
        value={query}
        onChange={(ev) => {
          setQuery(ev.target.value);
          setPicked(null);
        }}
        onKeyDown={keydown}
        type="text"
        role="combobox"
        aria-expanded="true"
        aria-controls="palette-list"
        aria-autocomplete="list"
        aria-activedescendant={current ? rowId(current) : undefined}
        aria-label="Search commands"
        placeholder="Search commands…"
        autoComplete="off"
        spellCheck={false}
        data-testid="palette-input"
        className="h-(--mf-size-control-wide) w-full border-b border-hairline bg-transparent px-4 text-body text-text-strong outline-none placeholder:text-text-muted"
      />
      <div id="palette-list" role="listbox" aria-label="Commands" data-testid="palette-list" className="max-h-80 overflow-y-auto p-1">
        {shown.map((e, i) => (
          // The keys of the list are the field's (see keydown); a row is for the pointer. A press must not take the focus from the field.
          <div
            key={e.id}
            id={rowId(e)}
            role="option"
            tabIndex={-1}
            aria-selected={i === at}
            aria-disabled={!e.enabled}
            data-testid="palette-option"
            data-id={e.id}
            onMouseDown={(ev) => ev.preventDefault()}
            onPointerMove={() => {
              if (e.enabled && i !== at) setPicked(e.id);
            }}
            onClick={() => e.enabled && onRun(e)}
            className={`${row} ${i === at ? 'bg-control' : ''} ${e.enabled ? 'cursor-default' : 'opacity-(--mf-opacity-disabled)'}`}
          >
            <span className="truncate">{e.label}</span>
            <span className="flex shrink-0 items-center gap-2 text-caption text-text-muted">
              <span>{e.group}</span>
              {e.detail ? <span className="font-mono">{e.detail}</span> : null}
              {e.shortcut ? <kbd className="rounded-xs border border-menu-border bg-field px-1 font-mono">{e.shortcut}</kbd> : null}
            </span>
          </div>
        ))}
      </div>
      {shown.length === 0 ? (
        <p className="m-0 px-4 py-3 text-small text-text-muted" data-testid="palette-empty">
          Nothing matches.
        </p>
      ) : null}
      <p className="sr-only" role="status">
        {resultsText(shown.length)}
      </p>
    </div>
  );
}
