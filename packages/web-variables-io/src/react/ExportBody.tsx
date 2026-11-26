// React copy of ExportVariablesBody.svelte. What is inside the Export variables dialog (D8): the one-line summary, the preview of the
// CSS with line numbers, the help, what could not be written as stored, and Close, Copy and Download. It is a function of what it is
// given: the dialog of the app layer (`state.variablesIo.export`) and what the last Copy said, which the shell of the dialog keeps.
// Every text from the file is drawn as plain text, without the characters that reorder it. A text with a value in it is one string
// (a template literal), as the Svelte copy draws it: React makes two text nodes of `Showing {n} of {m}`, and a browser shapes two runs.
import type { VariablesExportDialog as Model } from '@meldframe/protocol';
import { Button } from '@meldframe/web-chrome/react/Button';
import { EXPORT_FILE, EXPORT_HELP, exportProblem, exportSummary, findingRows, previewOf } from '../io';

export interface ExportBodyProps {
  dialog: Model;
  /** What the last Copy said. */
  said: { ok: boolean; text: string } | null;
  onCopy: () => void;
  onDownload: () => void;
  onClose: () => void;
}

export function ExportBody({ dialog, said, onCopy, onDownload, onClose }: ExportBodyProps) {
  const problem = exportProblem(dialog);
  const summary = exportSummary(dialog);
  const preview = previewOf(dialog.css);
  const empty = !problem && dialog.variables === 0;
  const warnings = findingRows(dialog.warnings);
  return (
    <>
      <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto px-4 pt-3">
        {problem ? (
          <p role="alert" data-testid="export-vars-error" className="m-0 rounded-md bg-danger-bg px-3 py-2 text-small text-danger-text">
            {problem}
          </p>
        ) : empty ? (
          <p data-testid="export-vars-empty" className="m-0 text-text-muted">
            This file has no variables yet, so there is nothing to export.
          </p>
        ) : (
          <>
            <p className="m-0 font-mono text-small text-text-muted" data-testid="export-vars-summary">
              {summary}
            </p>
            {/* A scrollable region must be reachable by keyboard. */}
            <div
              className="h-72 shrink-0 overflow-auto rounded-md border border-border bg-sidebar py-2 font-mono text-small leading-relaxed"
              data-testid="export-vars-code"
              tabIndex={0}
              role="region"
              aria-label={`${EXPORT_FILE} preview`}
            >
              {preview.lines.map((line, i) => (
                // The lines are a fixed list of text, never reordered: the position is the identity, as in the Svelte copy.
                <div key={i} className="flex w-max min-w-full whitespace-pre">
                  <span aria-hidden="true" className="sticky left-0 w-12 shrink-0 bg-sidebar pr-3 text-right text-text-muted select-none">
                    {i + 1}
                  </span>
                  <span className="pr-4">{line}</span>
                </div>
              ))}
            </div>
            {preview.total > preview.lines.length ? (
              <p className="m-0 text-caption text-text-muted" data-testid="export-vars-cut">
                {`Showing the first ${preview.lines.length} of ${preview.total} lines. Copy and Download take all of it.`}
              </p>
            ) : null}
          </>
        )}
        {EXPORT_HELP.map((line) => (
          <p key={line} className="m-0 text-caption text-text-muted" data-testid="export-vars-help">
            {line}
          </p>
        ))}
        {warnings.length ? (
          <section aria-label="Warnings" data-testid="export-vars-warnings" className="overflow-hidden rounded-md border border-border">
            <header className="flex h-(--mf-size-control-lg) items-center gap-2 bg-field px-3 text-small font-semibold text-text-strong">
              Warnings
              <span className="rounded-pill border border-border bg-warning-bg px-2 text-caption font-bold text-warning">{warnings.length}</span>
              <span className="ml-auto font-normal text-text-muted">Your file is unchanged.</span>
            </header>
            <ul className="m-0 max-h-32 list-none overflow-y-auto p-0">
              {warnings.map((r) => (
                <li key={r.key} className="flex items-baseline gap-3 border-t border-hairline px-3 py-2 text-small">
                  <span className="w-36 shrink-0 truncate font-mono text-warning">{r.code}</span>
                  <span className="min-w-0 flex-1 text-text-secondary">{r.message}</span>
                </li>
              ))}
            </ul>
          </section>
        ) : null}
      </div>
      <div className="flex shrink-0 items-center justify-end gap-2 px-4 pt-3 pb-4">
        <span role="status" className={`mr-auto text-small ${said?.ok === false ? 'text-danger' : 'text-text-muted'}`} data-testid="export-vars-said">
          {said?.text ?? ''}
        </span>
        <Button onClick={onClose} data-testid="export-vars-done">
          Close
        </Button>
        <Button disabled={!!problem || empty} onClick={onCopy} data-testid="export-vars-copy">
          Copy
        </Button>
        <Button variant="primary" disabled={!!problem || empty} onClick={onDownload} data-testid="export-vars-download">
          Download
        </Button>
      </div>
    </>
  );
}
