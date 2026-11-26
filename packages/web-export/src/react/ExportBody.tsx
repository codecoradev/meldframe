// React copy of ExportBody.svelte. What is inside the Export dialog (D9): the tabs, the HTML tab (style, file, the
// preview with line numbers, the report, Copy and Download) and the PNG tab. It is a function of what it is given: the
// dialog of the app layer, the view (tab, file shown, what the last Copy said) that the shell of the dialog keeps, and
// the PNG plan of the dialog's frame. Every text from the file (names, the report) is drawn as plain text, without
// the characters that reorder it. The rules (png, save, ui, view) are shared with the Svelte copy.
import type { Action, ExportDialog as Model } from '@meldframe/protocol';
import { Button } from '@meldframe/web-chrome/react/Button';
import { Segmented } from '@meldframe/web-chrome/react/Segmented';
import { Tabs } from '@meldframe/web-chrome/react/Tabs';
import type { ExportScale } from '@meldframe/web-document/export';
import { useRef } from 'react';
import { type PngPlan, SCALE_OPTIONS, pngText, scaleOf } from '../png';
import { copyText, saveText } from '../save';
import { type View, afterFormat } from '../ui';
import { errorText, fileOf, findingRows, plain, previewOf, reviewText, sizeText } from '../view';

export interface ExportBodyProps {
  dialog: Model;
  /** `state.activity.unreviewedCount`. */
  unreviewed: number;
  view: View;
  onView: (v: View) => void;
  scale: ExportScale;
  onScale: (s: ExportScale) => void;
  /** The PNG of the dialog's frame as the canvas would draw it now (null: nothing to draw). */
  plan: PngPlan | null;
  onPng: () => void;
  dispatch: (a: Action) => void;
  /** The dialog closes and the page opens the Activity tab. */
  onReview: () => void;
  onClose: () => void;
}

function Report({ title, rows, note, id }: { title: string; rows: ReturnType<typeof findingRows>; note: string; id: string }) {
  if (!rows.length) return null;
  return (
    <section aria-label={title} data-testid={id} className="overflow-hidden rounded-md border border-border">
      <header className="flex h-(--mf-size-control-lg) items-center gap-2 bg-field px-3 text-small font-semibold text-text-strong">
        {title}
        <span className="rounded-pill border border-border bg-warning-bg px-2 text-caption font-bold text-warning">{rows.length}</span>
        <span className="ml-auto font-normal text-text-muted">{note}</span>
      </header>
      <ul className="m-0 max-h-32 list-none overflow-y-auto p-0">
        {rows.map((r) => (
          <li key={r.key} className="flex items-baseline gap-3 border-t border-hairline px-3 py-2 text-small">
            <span className="w-32 shrink-0 truncate font-medium text-text-strong" title={r.node}>
              {r.node}
            </span>
            <span className="w-36 shrink-0 truncate font-mono text-warning">{r.code}</span>
            <span className="min-w-0 flex-1 text-text-secondary">{r.message}</span>
          </li>
        ))}
      </ul>
    </section>
  );
}

function TabStrip({ view, onView }: { view: View; onView: (v: View) => void }) {
  return (
    <div className="flex shrink-0 px-4 pt-3">
      <Tabs
        label="Export format"
        idPrefix="export"
        selected={view.tab}
        onSelect={(t) => onView({ ...view, tab: t === 'png' ? 'png' : 'html' })}
        tabs={[
          { id: 'html', label: 'HTML + Tailwind' },
          { id: 'png', label: 'PNG' },
        ]}
      />
    </div>
  );
}

export function ExportBody({ dialog, unreviewed, view, onView, scale, onScale, plan, onPng, dispatch, onReview, onClose }: ExportBodyProps) {
  // A plain CSS form has the stylesheet inside the HTML and no second file.
  const file = fileOf(dialog, dialog.format === 'tailwind' && view.shown === 'css' ? 'css' : 'html');
  const preview = previewOf(file.text);
  const problem = errorText(dialog);
  const review = reviewText(unreviewed);
  // Copy answers after the browser has: it reads the view as it is then, as the Svelte copy reads its prop, not as it was at the click.
  const latest = useRef(view);
  latest.current = view;

  async function copy() {
    const said = (await copyText(file.text)) ? { ok: true, text: `${file.name} copied.` } : { ok: false, text: 'Your browser did not allow copying.' };
    onView({ ...latest.current, said });
  }

  if (view.tab === 'png') {
    return (
      <>
        <TabStrip view={view} onView={onView} />
        <div id="export-panel-png" role="tabpanel" aria-labelledby="export-tab-png" className="flex flex-col gap-3 px-4 pt-4 pb-4">
          <div className="flex items-center justify-between">
            <span className="font-semibold text-text-strong">Scale</span>
            <Segmented label="Scale" options={SCALE_OPTIONS} value={String(scale)} onChange={(v) => onScale(scaleOf(v))} data-testid="export-scale" />
          </div>
          <p role="status" className={`m-0 text-small ${plan?.ok === false && plan.why === 'too_large' ? 'text-danger-text' : 'font-mono text-text-muted'}`} data-testid="export-png-plan">
            {plan ? pngText(plan) : 'There is nothing to export.'}
          </p>
          <div className="flex justify-end gap-2">
            <Button onClick={onClose}>Close</Button>
            <Button variant="primary" disabled={!plan?.ok} onClick={onPng} data-testid="export-png">
              Export as PNG
            </Button>
          </div>
        </div>
      </>
    );
  }

  return (
    <>
      <TabStrip view={view} onView={onView} />
      <div id="export-panel-html" role="tabpanel" aria-labelledby="export-tab-html" className="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto px-4 pt-3">
        <div className="flex items-center gap-3">
          <Segmented
            label="Style"
            options={[
              { value: 'tailwind', label: 'Tailwind' },
              { value: 'css', label: 'Plain CSS' },
            ]}
            value={dialog.format}
            onChange={(v) => {
              onView(afterFormat(view));
              dispatch({ type: 'setExportFormat', format: v === 'css' ? 'css' : 'tailwind' });
            }}
            data-testid="export-format"
          />
          {dialog.format === 'tailwind' && !problem ? (
            <Segmented
              label="File"
              options={[
                { value: 'html', label: 'HTML' },
                { value: 'css', label: plain(dialog.cssFile) },
              ]}
              value={view.shown}
              onChange={(v) => onView({ ...view, shown: v === 'css' ? 'css' : 'html', said: null })}
              data-testid="export-file"
            />
          ) : null}
          {!problem ? <span className="ml-auto font-mono text-small text-text-muted">{sizeText(file.bytes)}</span> : null}
        </div>
        {problem ? (
          <p role="alert" data-testid="export-error" className="m-0 rounded-md bg-danger-bg px-3 py-2 text-small text-danger-text">
            {problem}
          </p>
        ) : (
          <>
            {/* A scrollable region must be reachable by keyboard. */}
            <div className="h-72 shrink-0 overflow-auto rounded-md border border-border bg-sidebar py-2 font-mono text-small leading-relaxed" data-testid="export-code" tabIndex={0} role="region" aria-label={`${plain(file.name)} preview`}>
              {preview.lines.map((line, i) => (
                // The lines have no identity but their place, as in the Svelte copy.
                <div key={i} className="flex w-max min-w-full whitespace-pre">
                  <span aria-hidden="true" className="sticky left-0 w-12 shrink-0 bg-sidebar pr-3 text-right text-text-muted select-none">
                    {i + 1}
                  </span>
                  <span className="pr-4">{line}</span>
                </div>
              ))}
            </div>
            {preview.total > preview.lines.length ? (
              <p className="m-0 text-caption text-text-muted" data-testid="export-cut">
                {`Showing the first ${preview.lines.length} of ${preview.total} lines. Copy and Download take all of it.`}
              </p>
            ) : null}
            <p className="m-0 text-caption text-text-muted">
              {`The HTML uses the Inter font file inter.woff2 beside it${dialog.format === 'tailwind' ? `, and links ${plain(dialog.cssFile)}, the input of the Tailwind v4 compiler` : ''}. Text sizes are measured in this browser, with the font the canvas draws, so they are the sizes the canvas lays text out with. The exported page is laid out by the browser that opens it: a text with a tab, a link or CJK characters can break differently there (Warnings names the ones it detects), and any text can if the Inter file is missing.`}
            </p>
          </>
        )}
        <Report title="Unsupported properties" rows={findingRows(dialog.unsupported)} note="Left out or approximated. Your file is unchanged." id="export-unsupported" />
        <Report title="Warnings" rows={findingRows(dialog.warnings)} note="Your file is unchanged." id="export-warnings" />
      </div>
      <div className="flex shrink-0 flex-col gap-3 px-4 pt-3 pb-4">
        {review ? (
          <div role="status" data-testid="export-review" className="flex items-center gap-3 rounded-md bg-review-bg px-3 py-2 text-small text-review-text">
            <span className="size-2 shrink-0 rounded-pill bg-agent-1"></span>
            <span className="flex-1">{review}</span>
            <Button size="sm" onClick={onReview}>
              Review
            </Button>
          </div>
        ) : null}
        <div className="flex items-center justify-end gap-2">
          <span role="status" className={`mr-auto text-small ${view.said?.ok === false ? 'text-danger' : 'text-text-muted'}`} data-testid="export-said">
            {view.said?.text ?? ''}
          </span>
          <Button onClick={onClose}>Close</Button>
          <Button disabled={!!problem} onClick={copy} data-testid="export-copy">
            Copy
          </Button>
          <Button variant="primary" disabled={!!problem} onClick={() => saveText(file.text, file.name, file.mime)} data-testid="export-download">
            Download
          </Button>
        </div>
      </div>
    </>
  );
}
