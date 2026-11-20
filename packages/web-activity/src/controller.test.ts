import { describe, expect, it } from 'vitest';
import type { Checkpoint } from '@meldframe/protocol';
import { activity, lane, row } from './fixtures';
import { askRestore, askRevert, cancel, confirmRestore, confirmRevert, laneOpen, settle, show, start, toggleLane } from './controller';

const a = row({ actor: 'agent:a', keyPrefix: 'ka', n: 1 });
const b = row({ actor: 'agent:b', keyPrefix: 'kb', n: 2 });
const refA = { actor: 'agent:a', keyPrefix: 'ka' };
const refB = { actor: 'agent:b', keyPrefix: 'kb' };
const cp = (id: string): Checkpoint => ({ id, seq: 1, label: id, kind: 'manual' });
const state = (selected: typeof refA | null = null) => activity({ lanes: [lane({ actor: 'agent:a', batches: ['ka'] }), lane({ actor: 'agent:b', batches: ['kb'] })], rows: [a, b], selected });

describe('the revert confirmation', () => {
  it('asking selects the batch and opens the confirmation; confirming sends the revert once', () => {
    const asked = askRevert(start, a);
    expect(asked.actions).toEqual([{ type: 'selectBatch', batch: refA }]);
    expect(asked.ui.confirm).toEqual({ kind: 'revert', ...refA });
    const done = confirmRevert(asked.ui, a, refA);
    expect(done.actions).toEqual([{ type: 'revertBatch', actor: 'agent:a', keyPrefix: 'ka' }]);
    expect(done.ui.confirm).toBeNull();
  });

  it('a second click on Confirm, before the page redraws, sends nothing: one revert per confirmation', () => {
    const asked = askRevert(start, a);
    const first = confirmRevert(asked.ui, a, refA);
    expect(confirmRevert(first.ui, a, refA).actions).toEqual([]);
  });

  it('a second click on Revert asks again, which only selects the same batch again', () => {
    const once = askRevert(start, a);
    const twice = askRevert(once.ui, a);
    expect(twice.actions).toEqual(once.actions);
    expect(twice.ui.confirm).toEqual(once.ui.confirm);
  });

  it('Confirm after the selection moved to another batch sends nothing, because its in-effect answer was for the other one', () => {
    const asked = askRevert(start, a);
    expect(confirmRevert(asked.ui, a, refB).actions).toEqual([]);
    expect(confirmRevert(asked.ui, a, null).actions).toEqual([]);
  });

  it('Confirm of a batch other than the one asked about sends nothing', () => {
    const asked = askRevert(start, a);
    expect(confirmRevert(asked.ui, b, refB).actions).toEqual([]);
    // even while the asked-about batch is the selected one
    expect(confirmRevert(asked.ui, b, refA).actions).toEqual([]);
  });

  it('Esc and Cancel close it and send nothing', () => {
    const asked = askRevert(start, a);
    expect(cancel(asked.ui)).toEqual({ ui: start, actions: [] });
  });

  it('Show on another batch while the confirmation is open closes it and selects the other batch', () => {
    const asked = askRevert(start, a);
    const moved = show(asked.ui, b);
    expect(moved.actions).toEqual([{ type: 'selectBatch', batch: refB }]);
    expect(moved.ui.confirm).toBeNull();
  });

  it('Show on the same batch keeps its confirmation open', () => {
    const asked = askRevert(start, a);
    expect(show(asked.ui, a).ui.confirm).toEqual(asked.ui.confirm);
  });

  it('settle closes the confirmation when the selection moved by any other way, when the row went, and keeps it while it holds', () => {
    const asked = askRevert(start, a).ui;
    expect(settle(asked, state(refA), [])).toBe(asked);
    expect(settle(asked, state(refB), []).confirm).toBeNull();
    expect(settle(asked, state(null), []).confirm).toBeNull();
    expect(settle(asked, activity({ rows: [b], selected: refA }), []).confirm).toBeNull();
  });
});

describe('the restore confirmation', () => {
  it('asks, then sends the restore once', () => {
    const asked = askRestore(start, 'c1');
    expect(asked.actions).toEqual([]);
    const done = confirmRestore(asked.ui, 'c1');
    expect(done.actions).toEqual([{ type: 'restoreCheckpoint', id: 'c1' }]);
    expect(confirmRestore(done.ui, 'c1').actions).toEqual([]);
  });

  it('confirming another checkpoint than the one asked about sends nothing', () => {
    expect(confirmRestore(askRestore(start, 'c1').ui, 'c2').actions).toEqual([]);
  });

  it('there is one confirmation at a time: asking to restore closes a revert confirmation, and the other way round', () => {
    const revert = askRevert(start, a).ui;
    expect(askRestore(revert, 'c1').ui.confirm).toEqual({ kind: 'restore', id: 'c1' });
    expect(askRevert(askRestore(start, 'c1').ui, a).ui.confirm?.kind).toBe('revert');
  });

  it('settle closes it when the checkpoint is gone', () => {
    const asked = askRestore(start, 'c1').ui;
    expect(settle(asked, state(), [cp('c1')])).toBe(asked);
    expect(settle(asked, state(), [cp('c2')]).confirm).toBeNull();
  });
});

describe('lanes open and fold', () => {
  it('opens the first lane and folds the others until the person chooses', () => {
    expect([0, 1, 2].map((i) => laneOpen(start, `agent:${i}`, i))).toEqual([true, false, false]);
  });

  it('keeps the person\'s choice for an actor when the order of the lanes changes', () => {
    // the person folded the first lane and opened the second
    let ui = toggleLane(start, 'agent:a', laneOpen(start, 'agent:a', 0)).ui;
    ui = toggleLane(ui, 'agent:b', laneOpen(ui, 'agent:b', 1)).ui;
    expect(laneOpen(ui, 'agent:a', 0)).toBe(false);
    expect(laneOpen(ui, 'agent:b', 1)).toBe(true);
    // agent:b becomes the most active and moves first: nothing flips
    expect(laneOpen(ui, 'agent:b', 0)).toBe(true);
    expect(laneOpen(ui, 'agent:a', 1)).toBe(false);
  });

  it('leaves the rest of the state alone when a lane is toggled', () => {
    const asked = askRevert(start, a).ui;
    expect(toggleLane(asked, 'agent:a', true).ui.confirm).toEqual(asked.confirm);
  });
});
