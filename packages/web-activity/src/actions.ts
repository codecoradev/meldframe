// What the panel's controls send. Pure, so the wiring of a button is a test and not a promise: the Svelte files only
// call these (`controller.ts` puts them in sequence, with the confirmations). Nothing here decides who may do what; the app layer refuses what it must (`Event::Refused`).
import type { Action, BatchRef, BatchRow, ReviewState } from '@meldframe/protocol';

export const refOf = (r: Pick<BatchRow, 'actor' | 'keyPrefix'>): BatchRef => ({ actor: r.actor, keyPrefix: r.keyPrefix });

/** Show: select the batch's nodes on the canvas. */
export const show = (r: BatchRow): Action => ({ type: 'selectBatch', batch: refOf(r) });

/** The revert itself, after the person confirmed it. */
export const revert = (r: BatchRow): Action => ({ type: 'revertBatch', actor: r.actor, keyPrefix: r.keyPrefix });

export const restore = (id: string): Action => ({ type: 'restoreCheckpoint', id });

export const older = (): Action => ({ type: 'loadOlderBatches' });

/** A review state for a batch. The app layer works out its number; the server decides whether this person may set it. */
export const setReview = (r: BatchRow, state: ReviewState): Action => ({ type: 'setBatchReview', batch: refOf(r), state });
