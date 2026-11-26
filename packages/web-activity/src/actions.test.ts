import { describe, expect, it } from 'vitest';
import { askRestore, askRevert, cancel, confirmRestore, confirmRevert, createCheckpoint, older, refOf, setReview, show } from './actions';
import { row } from './fixtures';

describe('what the controls send', () => {
  const r = row({ actor: 'agent:claude', keyPrefix: 'k7', n: 7 });
  const batch = { actor: 'agent:claude', keyPrefix: 'k7' };
  it('Show selects the batch by its actor and key prefix, and not by its number', () => {
    expect(show(r)).toEqual({ type: 'selectBatch', batch });
    expect(refOf(r)).toEqual(batch);
  });
  // Whether the question is open, for which batch and whether it is still the selected one are the app layer's (confirm.rs).
  it('Revert asks the question first, and Confirm answers it, both naming the batch by its actor and key prefix', () => {
    expect(askRevert(r)).toEqual({ type: 'activity', op: { type: 'askRevert', batch } });
    expect(confirmRevert(r)).toEqual({ type: 'activity', op: { type: 'confirmRevert', batch } });
  });
  it('Restore asks and confirms by the id of the checkpoint, and Cancel closes the question', () => {
    expect(askRestore('c1')).toEqual({ type: 'activity', op: { type: 'askRestore', id: 'c1' } });
    expect(confirmRestore('c1')).toEqual({ type: 'activity', op: { type: 'confirmRestore', id: 'c1' } });
    expect(cancel()).toEqual({ type: 'activity', op: { type: 'cancel' } });
  });
  it('asks for older batches without arguments', () => {
    expect(older()).toEqual({ type: 'loadOlderBatches' });
  });
  it('Create checkpoint sends the label as it is typed, trimmed; an empty one is no label', () => {
    expect(createCheckpoint('  Before the pitch ')).toEqual({ type: 'createCheckpoint', label: 'Before the pitch' });
    expect(createCheckpoint('   ')).toEqual({ type: 'createCheckpoint', label: '' });
  });
  it('sets a review state by actor and key prefix, which the app layer turns into the number', () => {
    expect(setReview(r, 'needs_review')).toEqual({ type: 'setBatchReview', batch, state: 'needs_review' });
  });
  it('sends each click: setting the state a row has is a no-op the server answers, so a double click is harmless', () => {
    expect([setReview(r, 'accepted'), setReview(r, 'accepted')]).toHaveLength(2);
  });
});
