// What the sections of the Properties panel read in common, apart from the markup (fields.ts): both copies, Svelte and React, call these.
import type { Action, Properties } from '@meldframe/protocol';
import { describe, expect, it } from 'vitest';
import { alignOf, pickerClosed, round2, sender, weightOf } from './fields';

const some = (over: Partial<Properties>) => over as Properties;

describe('round2', () => {
  it('keeps at most two decimals and drops a trailing zero', () => {
    expect(round2(10)).toBe('10');
    expect(round2(10.5)).toBe('10.5');
    expect(round2(3.14159)).toBe('3.14');
    expect(round2(0)).toBe('0');
    expect(round2(2.676)).toBe('2.68');
  });
});

describe('sender', () => {
  it('dispatches an action and swallows null (a field that changed nothing)', () => {
    const sent: Action[] = [];
    const send = sender((a) => sent.push(a));
    send(null);
    send({ type: 'undo' } as Action);
    expect(sent).toEqual([{ type: 'undo' }]);
  });
});

describe('the defaults of a text', () => {
  it('weight 400 and left for a node that has none, the node\'s own otherwise', () => {
    expect(weightOf(some({}))).toBe('400');
    expect(weightOf(some({ fontWeight: 700 }))).toBe('700');
    expect(alignOf(some({}))).toBe('left');
    expect(alignOf(some({ textAlign: 'right' }))).toBe('right');
  });
});

describe('pickerClosed (where the focus goes after the image picker closes)', () => {
  it('is true only when the picker was open for this node and is not now', () => {
    expect(pickerClosed('n', 'n', false)).toBe(true);
    expect(pickerClosed('n', 'n', true)).toBe(false);
    expect(pickerClosed(null, 'n', false)).toBe(false);
    // Another node was selected: the picker closed with the selection, and focus is left alone.
    expect(pickerClosed('old', 'new', false)).toBe(false);
  });
});
