import { describe, expect, it } from 'vitest';
import { aboutAfter, eventsSay, isSubmitKey, laneOpen, start, toggleLane } from './controller';
import { checkpointMade } from './view';
import { cancel, confirmRestore, confirmRevert, createCheckpoint, older, setReview } from './actions';
import { row } from './fixtures';

// The question being asked (revert, restore), the settling of it and the checkpoint a person asked for are the app layer's:
// crates/core/meldframe-app-review-tests/tests/confirm.rs. What stays here is which lanes are open.
describe('lanes open and fold', () => {
  it('opens the first lane and folds the others until the person chooses', () => {
    expect([laneOpen(start, 'a', 0), laneOpen(start, 'b', 1), laneOpen(start, 'c', 2)]).toEqual([true, false, false]);
  });
  it("keeps the person's choice for an actor when the order of the lanes changes", () => {
    const ui = toggleLane(toggleLane(start, 'b', false), 'a', true);
    expect(laneOpen(ui, 'b', 0)).toBe(true);
    expect(laneOpen(ui, 'a', 1)).toBe(false);
    expect(laneOpen(ui, 'b', 5)).toBe(true);
  });
  it('leaves the choices of the other lanes alone when one is toggled', () => {
    const one = toggleLane(start, 'a', true);
    const two = toggleLane(one, 'b', false);
    expect(two.lanes).toEqual({ a: false, b: true });
    expect(one.lanes).toEqual({ a: false });
  });
});

describe('Create checkpoint', () => {
  it('Enter submits, and not while an input method is composing, nor with Shift', () => {
    const key = (k: string, over = {}) => isSubmitKey({ key: k, isComposing: false, shiftKey: false, ...over });
    expect(key('Enter')).toBe(true);
    expect(key('Enter', { isComposing: true })).toBe(false);
    expect(key('Enter', { shiftKey: true })).toBe(false);
    expect(key('a')).toBe(false);
  });
});

// What the panel does with the events of a dispatch is shared by the Svelte and the React copy: one rule, tested here.
describe('what the last request was about', () => {
  const r = row();
  it('is a checkpoint after a Create checkpoint, a confirmed revert or a confirmed restore, so a refusal is worded for it', () => {
    expect(aboutAfter(undefined, createCheckpoint('x'))).toBe('checkpoint');
    expect(aboutAfter(undefined, confirmRevert(r))).toBe('checkpoint');
    expect(aboutAfter(undefined, confirmRestore('c1'))).toBe('checkpoint');
  });
  it('goes back to a review after a review, so a refusal of the next one is worded for a review', () => {
    expect(aboutAfter('checkpoint', setReview(r, 'accepted'))).toBeUndefined();
  });
  it('stays as it was for any other action', () => {
    expect(aboutAfter('checkpoint', cancel())).toBe('checkpoint');
    expect(aboutAfter(undefined, older())).toBeUndefined();
  });
});

describe('what the events of one dispatch say to the panel', () => {
  it('says a checkpoint was made, and empties the field, whatever else came with it', () => {
    expect(eventsSay([{ type: 'refused', code: 'forbidden' }, { type: 'checkpointMade', key: 'k' }], undefined)).toEqual({ made: true, message: checkpointMade });
  });
  it('words a refusal by what the person asked for last', () => {
    const refused = [{ type: 'refused', code: 'forbidden' } as const];
    expect(eventsSay(refused, undefined).message?.text).toMatch(/accept or flag batches/);
    expect(eventsSay(refused, 'checkpoint').message?.text).toMatch(/make or restore checkpoints/);
    expect(eventsSay(refused, 'checkpoint').made).toBe(false);
  });
  it('says nothing for events it has no words for', () => {
    expect(eventsSay([], undefined)).toEqual({ made: false, message: null });
  });
});
