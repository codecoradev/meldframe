// React copy of BatchItem.svelte. One batch of a lane: its number, how far it got, the time, what it did, the agent's note, and Show
// and Revert. The note is text an agent wrote: it is drawn as text only (no markup, no links), whole (at most 500 characters and 3
// line breaks, so it cannot take over the card), attributed to its full actor and in quotation marks, and a bidirectional override
// inside it cannot reach the text around it (<bdi>). Which row may be reverted, and whether a revert would still change anything,
// is what the app layer says (`revertable`, `inEffect`). What a click sends is `actions.ts`. A text with a value in it is one string.
import type { BatchRow, ReviewState } from '@meldframe/protocol';
import { Button } from '@meldframe/web-chrome/react/Button';
import { useEffect, useRef } from 'react';
import { batchHeading, clockText, reviewChoices, reviewedBy, reviewWords, statusWords, summaryWords } from '../view';

export interface BatchItemProps {
  row: BatchRow;
  selected: boolean;
  /** The confirmation of a revert is open on this row. */
  confirming: boolean;
  /** Whether the server lets this person set a review state (`canReview`): the controls show only then. */
  canReview: boolean;
  onReview: (s: ReviewState) => void;
  onShow: () => void;
  onAskRevert: () => void;
  onRevert: () => void;
  onCancel: () => void;
}

export function BatchItem({ row, selected, confirming, canReview, onReview, onShow, onAskRevert, onRevert, onCancel }: BatchItemProps) {
  const heading = batchHeading(row);
  const summary = summaryWords(row.summary);
  const choices = reviewChoices(row, canReview);
  const by = reviewedBy(row);
  const revertButton = useRef<HTMLSpanElement>(null);
  const cancelButton = useRef<HTMLSpanElement>(null);

  // Keyboard: the confirmation takes the focus (on its safe button) when it opens, and gives it back to Revert when the person closes it.
  useEffect(() => {
    if (confirming) cancelButton.current?.querySelector('button')?.focus();
  }, [confirming]);
  // The Svelte copy waits for `tick()` and then focuses Revert; this waits for the next task. The assumption it relies on: the island draws
  // what the dispatch changed through `flushSync` before the dispatch returns, so that draw is over by the next task; and Revert is on
  // the row before, during and after the question, so it is there to take the focus whether the app layer cleared the question in that
  // dispatch or keeps it until a checkpoint it waits for is made. activity-e2e.mjs asks for the focus after a confirmed revert and restore.
  // A timer left over from a row that is gone is cleared with the row.
  const focusTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  useEffect(() => () => clearTimeout(focusTimer.current), []);
  const close = (then: () => void) => () => {
    then();
    clearTimeout(focusTimer.current);
    focusTimer.current = setTimeout(() => revertButton.current?.querySelector('button')?.focus(), 0);
  };

  return (
    <li
      className={`flex flex-col gap-2 rounded-md border px-3 py-2 ${selected ? 'border-selection bg-selection-bg' : 'border-hairline bg-field'}`}
      data-testid="batch"
      data-n={row.n ?? undefined}
      data-status={row.status}
      aria-current={selected ? 'true' : undefined}
    >
      <div className="flex items-baseline justify-between gap-2">
        <span className="text-body font-semibold text-text-strong">{heading}</span>
        <span className="text-caption text-text-muted">{clockText(row.atMs)}</span>
      </div>
      <p className="m-0 text-small text-text-secondary" data-testid="batch-status">
        {statusWords(row)}
      </p>
      {row.inEffect === false ? (
        <p className="m-0 text-small text-text-secondary" data-testid="batch-not-in-effect">
          Not in effect any more: what it wrote was written again since, or it was reverted.
        </p>
      ) : null}
      {row.n !== null ? (
        <div className="flex flex-wrap items-center gap-2">
          <span className={`rounded-pill px-2 text-caption font-medium ${row.review === 'needs_review' ? 'bg-review-bg text-review-text' : 'bg-control text-text-secondary'}`} data-testid="batch-review">
            {reviewWords(row.review)}
          </span>
          {by ? (
            <span className="min-w-0 break-words text-caption text-text-muted" data-testid="batch-review-by">
              Last changed by <bdi>{by.by}</bdi>
              {by.at ? ` at ${by.at}` : ''}
            </span>
          ) : null}
        </div>
      ) : null}
      {summary ? (
        <p className="m-0 text-small text-text break-words" data-testid="batch-summary">
          {summary}
        </p>
      ) : null}
      {row.note ? (
        <figure className="m-0 flex flex-col gap-1" data-testid="batch-note">
          <figcaption className="text-caption text-text-muted break-words">
            Agent note from <bdi>{row.actor}</bdi>
          </figcaption>
          <blockquote className="m-0 border-l-2 border-border-strong pl-2 text-small text-text break-words whitespace-pre-line">
            “<bdi>{row.note}</bdi>”
          </blockquote>
        </figure>
      ) : null}
      <div className="flex flex-wrap items-center gap-2">
        <Button size="sm" disabled={!row.canShow} onClick={onShow} data-testid="batch-show">
          Show
        </Button>
        {row.revertable ? (
          <span ref={revertButton} className="contents">
            <Button size="sm" variant="secondary" onClick={onAskRevert} data-testid="batch-revert">
              Revert
            </Button>
          </span>
        ) : row.n !== null ? (
          <span className="text-caption text-text-muted" data-testid="batch-not-revertable">
            Cannot be reverted from this page.
          </span>
        ) : null}
      </div>
      {choices.length ? (
        <div className="flex flex-wrap items-center gap-2" role="group" aria-label={`Review ${heading}`}>
          {choices.map((c) => (
            <Button key={c.state} size="sm" aria-label={`${c.label}: ${heading}`} data-testid="review-set" data-state={c.state} onClick={() => onReview(c.state)}>
              {c.label}
            </Button>
          ))}
        </div>
      ) : null}
      {confirming ? (
        // Escape closes the confirmation from any control inside it; the group is not a control itself.
        <div
          role="group"
          aria-label={`Confirm the revert of ${heading}`}
          className="flex flex-col gap-2 rounded-md border border-border bg-surface px-3 py-2"
          data-testid="revert-confirm"
          onKeyDown={(e) => {
            if (e.key === 'Escape') {
              e.stopPropagation();
              close(onCancel)();
            }
          }}
        >
          {row.inEffect === false ? (
            <>
              <p className="m-0 text-small text-text">Nothing left to revert: every field this batch wrote was written again since, or it was reverted already.</p>
              <span ref={cancelButton} className="contents">
                <Button size="sm" onClick={close(onCancel)} data-testid="revert-cancel">
                  Close
                </Button>
              </span>
            </>
          ) : (
            <>
              <p className="m-0 text-small text-text">{`Revert ${heading}? What its agent wrote last goes back, as a new batch of yours. What others wrote since stays.`}</p>
              <div className="flex gap-2">
                <Button size="sm" variant="destructive" onClick={close(onRevert)} data-testid="revert-confirm-yes">
                  Revert
                </Button>
                <span ref={cancelButton} className="contents">
                  <Button size="sm" onClick={close(onCancel)} data-testid="revert-cancel">
                    Cancel
                  </Button>
                </span>
              </div>
            </>
          )}
        </div>
      ) : null}
    </li>
  );
}
