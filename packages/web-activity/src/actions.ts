// What the panel's controls send. Pure, so the wiring of a button is a test and not a promise: the Svelte files only
// call these. Nothing here decides who may do what or whether a question is still open; the app layer refuses what it
// must (`Event::Refused`) and keeps the question (`AppState.confirm`).
import type { Action, BatchRef, BatchRow, ReviewState } from '@meldframe/protocol';

export const refOf = (r: Pick<BatchRow, 'actor' | 'keyPrefix'>): BatchRef => ({ actor: r.actor, keyPrefix: r.keyPrefix });

/** Show: select the batch's nodes on the canvas. */
export const show = (r: BatchRow): Action => ({ type: 'selectBatch', batch: refOf(r) });

/** Revert: ask the question first (the app selects the batch and holds the question). */
export const askRevert = (r: BatchRow): Action => ({ type: 'activity', op: { type: 'askRevert', batch: refOf(r) } });

/** The revert itself, after the person confirmed it: the app goes ahead only for the batch it asked about. */
export const confirmRevert = (r: BatchRow): Action => ({ type: 'activity', op: { type: 'confirmRevert', batch: refOf(r) } });

export const askRestore = (id: string): Action => ({ type: 'activity', op: { type: 'askRestore', id } });

export const confirmRestore = (id: string): Action => ({ type: 'activity', op: { type: 'confirmRestore', id } });

/** Cancel, Close and Esc. */
export const cancel = (): Action => ({ type: 'activity', op: { type: 'cancel' } });

/** Create checkpoint: the label as typed, trimmed (empty is no label); the app checks it again before anything is sent. */
export const createCheckpoint = (label: string): Action => ({ type: 'createCheckpoint', label: label.trim() });

export const older = (): Action => ({ type: 'loadOlderBatches' });

/** A review state for a batch. The app layer works out its number; the server decides whether this person may set it. */
export const setReview = (r: BatchRow, state: ReviewState): Action => ({ type: 'setBatchReview', batch: refOf(r), state });
