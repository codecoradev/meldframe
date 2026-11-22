import { describe, expect, it } from 'vitest';
import { createCheckpoint, older, refOf, restore, revert, setReview, show } from './actions';
import { row } from './fixtures';

describe('what the controls send', () => {
  const r = row({ actor: 'agent:claude', keyPrefix: 'k7', n: 7 });
  it('Show selects the batch by its actor and key prefix, and not by its number', () => {
    expect(show(r)).toEqual({ type: 'selectBatch', batch: { actor: 'agent:claude', keyPrefix: 'k7' } });
    expect(refOf(r)).toEqual({ actor: 'agent:claude', keyPrefix: 'k7' });
  });
  it('Revert names the actor and key prefix of the batch', () => {
    expect(revert(r)).toEqual({ type: 'revertBatch', actor: 'agent:claude', keyPrefix: 'k7' });
  });
  it('restores a checkpoint by id and asks for older batches without arguments', () => {
    expect(restore('c1')).toEqual({ type: 'restoreCheckpoint', id: 'c1' });
    expect(older()).toEqual({ type: 'loadOlderBatches' });
  });
  it('Create checkpoint sends the label as it is typed, trimmed; an empty one is no label', () => {
    expect(createCheckpoint('  Before the pitch ')).toEqual({ type: 'createCheckpoint', label: 'Before the pitch' });
    expect(createCheckpoint('   ')).toEqual({ type: 'createCheckpoint', label: '' });
  });
  it('sets a review state by actor and key prefix, which the app layer turns into the number', () => {
    expect(setReview(r, 'needs_review')).toEqual({ type: 'setBatchReview', batch: { actor: 'agent:claude', keyPrefix: 'k7' }, state: 'needs_review' });
  });
  it('sends each click: setting the state a row has is a no-op the server answers, so a double click is harmless', () => {
    expect([setReview(r, 'accepted'), setReview(r, 'accepted')]).toHaveLength(2);
  });
});
