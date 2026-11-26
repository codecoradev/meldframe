// React copy of ErrorPage.svelte. The page for a file that did not open, or an editor that could not start: what
// happened, whether the file changed and what to do, as the ErrorPages board asks. The detail is text from the server or
// the browser, drawn as plain text.
import { useEffect, useRef } from 'react';
import { Button } from '@meldframe/web-chrome/react/Button';
import type { ErrorPageView } from '../failure';

export interface ErrorPageProps {
  page: ErrorPageView;
  onAction: () => void;
}

export function ErrorPage({ page, onAction }: ErrorPageProps) {
  const heading = useRef<HTMLHeadingElement>(null);
  // This page replaces the editor, so a reader is told (the alert) and the focus starts at the heading.
  useEffect(() => {
    heading.current?.focus();
  }, []);
  return (
    <main data-testid="error-page" data-kind={page.kind} className="fixed inset-0 grid place-items-center overflow-auto bg-canvas p-8 font-sans text-body text-text">
      <div className="flex w-full max-w-md flex-col gap-3 rounded-lg border border-border bg-surface p-6 shadow-float">
        <div role="alert" className="flex flex-col gap-3">
          <h1 ref={heading} tabIndex={-1} className="m-0 text-heading font-semibold text-text-strong outline-none">
            {page.title}
          </h1>
          <p className="m-0 text-text-secondary">{page.text}</p>
        </div>
        {page.detail ? (
          <p data-testid="error-detail" className="m-0 max-h-40 overflow-auto rounded-md bg-field p-3 font-mono text-small wrap-anywhere whitespace-pre-wrap text-text-muted">
            {page.detail}
          </p>
        ) : null}
        {page.action ? (
          <div className="flex justify-end">
            <Button variant="primary" onClick={onAction}>
              {page.action}
            </Button>
          </div>
        ) : null}
      </div>
    </main>
  );
}
