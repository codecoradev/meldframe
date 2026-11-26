// React copy of ImportVariablesBody.svelte. What is inside the Import variables dialog (D8): the field for the CSS text, the file
// chooser, the help, the result of the last import (what was added, or why nothing was) and its report, and Close and Import. It is a
// function of what it is given: the text of the field (the shell owns it), the note about the file, and the result of the app layer
// (`state.variablesIo.import`). A file is handed to `onFile`; reading it is the shell's. Every text from the file is drawn as plain
// text, without the characters that reorder it, as the app layer made it: nothing here cleans it again.
import type { VariablesImportResult } from '@meldframe/protocol';
import { Button } from '@meldframe/web-chrome/react/Button';
import { useRef } from 'react';
import { IMPORT_HELP, entryRows, importWords, omittedText } from '../io';

export interface ImportBodyProps {
  result: VariablesImportResult | null;
  text: string;
  onText: (text: string) => void;
  /** What the last file chosen came to: read, or why not. */
  note: { ok: boolean; text: string } | null;
  onFile: (file: File) => void;
  onClear: () => void;
  onImport: () => void;
  onClose: () => void;
}

// Whole class strings, never built from a prop, so that Tailwind finds them.
const TONE = { ok: 'bg-field text-text-strong', neutral: 'bg-field text-text', error: 'bg-danger-bg text-danger-text' } as const;
const BADGE = { converted: 'bg-warning-bg text-warning', skipped: 'bg-field text-text-muted', note: 'bg-field text-text-secondary' } as const;

export function ImportBody({ result, text, onText, note, onFile, onClear, onImport, onClose }: ImportBodyProps) {
  const chooser = useRef<HTMLInputElement>(null);
  const words = result ? importWords(result) : null;
  const rows = result?.report ? entryRows(result.report.entries) : [];
  const omitted = result?.report ? omittedText(result.report.omitted) : null;
  return (
    <>
      <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto px-4 pt-3">
        <label htmlFor="import-vars-text" className="text-small font-semibold text-text-strong">
          CSS to import
        </label>
        <textarea
          id="import-vars-text"
          value={text}
          onChange={(e) => onText(e.currentTarget.value)}
          data-testid="import-vars-text"
          rows={10}
          spellCheck={false}
          placeholder="Paste CSS with a :root, .dark or @theme block"
          className="min-h-40 w-full resize-y rounded-md border border-divider bg-field p-3 font-mono text-small text-text hov:border-border-strong foc:border-selection foc:outline-(length:--mf-stroke-focus) foc:outline-offset-(--mf-stroke-focus-offset) foc:outline-selection"
          // The keys typed here are text, not commands of the page; Escape is the dialog's.
          onKeyDown={(e) => {
            if (e.key !== 'Escape') e.stopPropagation();
          }}
        />
        <div className="flex items-center gap-3">
          <input
            ref={chooser}
            type="file"
            accept=".css,text/css,text/plain"
            className="sr-only"
            tabIndex={-1}
            aria-hidden="true"
            data-testid="import-vars-file"
            onChange={(e) => {
              const file = e.currentTarget.files?.[0];
              // Emptied, so that the same file chosen again is a change.
              e.currentTarget.value = '';
              if (file) onFile(file);
            }}
          />
          <Button onClick={() => chooser.current?.click()} data-testid="import-vars-choose">
            Choose a file…
          </Button>
          <Button disabled={text === ''} onClick={onClear} data-testid="import-vars-clear">
            Clear
          </Button>
          <span role="status" className={`min-w-0 flex-1 truncate text-small ${note?.ok === false ? 'text-danger' : 'text-text-muted'}`} data-testid="import-vars-note">
            {note?.text ?? ''}
          </span>
        </div>
        {IMPORT_HELP.map((line) => (
          <p key={line} className="m-0 text-caption text-text-muted" data-testid="import-vars-help">
            {line}
          </p>
        ))}
        {/* One live region for the result, error or not: a status inside an alert would be read twice. */}
        <div role="status" aria-live="polite" data-testid="import-vars-result" data-status={result?.status ?? ''}>
          {words && result ? (
            <>
              <div className={`flex flex-col gap-1 rounded-md px-3 py-2 text-small ${TONE[words.tone]}`}>
                <p className="m-0 flex items-center gap-2 font-semibold" data-testid="import-vars-title">
                  {words.tone === 'ok' ? <span className="size-2 shrink-0 rounded-pill bg-success" aria-hidden="true" /> : null}
                  {words.title}
                </p>
                {words.lines.map((line) => (
                  <p key={line} className="m-0" data-testid="import-vars-line">
                    {line}
                  </p>
                ))}
              </div>
              {rows.length ? (
                <section aria-label="Import report" data-testid="import-vars-report" className="mt-3 overflow-hidden rounded-md border border-border">
                  <header className="flex h-(--mf-size-control-lg) items-center gap-2 bg-field px-3 text-small font-semibold text-text-strong">
                    Report
                    <span className="rounded-pill border border-border px-2 text-caption font-bold">{rows.length}</span>
                  </header>
                  <ul className="m-0 max-h-48 list-none overflow-y-auto p-0">
                    {rows.map((r) => (
                      <li key={r.key} className="flex items-baseline gap-3 border-t border-hairline px-3 py-2 text-small" data-kind={r.kind}>
                        <span className={`w-20 shrink-0 rounded-pill px-2 text-center text-caption font-semibold ${BADGE[r.kind]}`}>{r.kind}</span>
                        <span className="w-36 shrink-0 truncate font-mono text-text-strong" title={r.name}>
                          {r.name}
                        </span>
                        <span className="min-w-0 flex-1 text-text-secondary">{r.message}</span>
                      </li>
                    ))}
                  </ul>
                  {omitted ? (
                    <p className="m-0 border-t border-hairline px-3 py-2 text-caption text-text-muted" data-testid="import-vars-omitted">
                      {omitted}
                    </p>
                  ) : null}
                </section>
              ) : null}
            </>
          ) : null}
        </div>
      </div>
      <div className="flex shrink-0 items-center justify-end gap-2 px-4 pt-3 pb-4">
        <Button onClick={onClose} data-testid="import-vars-done">
          Close
        </Button>
        <Button variant="primary" disabled={text.trim() === ''} onClick={onImport} data-testid="import-vars-import">
          Import
        </Button>
      </div>
    </>
  );
}
