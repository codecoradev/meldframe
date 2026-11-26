import { describe, expect, it } from 'vitest';
import { isSubmitKey, laneOpen, start, toggleLane } from './controller';

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
