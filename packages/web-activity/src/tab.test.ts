import { describe, expect, it } from 'vitest';
import type { Event } from '@meldframe/protocol';
import { tabChange, toastFor } from './tab';

describe('tabChange', () => {
  it('opening the Activity tab asks the app layer for the newest batches', () => {
    expect(tabChange('design', 'activity')).toEqual({ tab: 'activity', actions: [{ type: 'openActivity' }] });
  });
  it('selecting the tab that is already selected asks for nothing, so a second click or a key on it does not fetch again', () => {
    expect(tabChange('activity', 'activity')).toEqual({ tab: 'activity', actions: [] });
    expect(tabChange('design', 'design')).toEqual({ tab: 'design', actions: [] });
  });
  it('going back to Design asks for nothing', () => {
    expect(tabChange('activity', 'design')).toEqual({ tab: 'design', actions: [] });
  });
});

describe('toastFor', () => {
  const refused = (code: string): Event => ({ type: 'refused', code });
  it('words a refusal in the toast when the panel is not showing', () => {
    expect(toastFor([refused('forbidden')], false)).toBe('This change is not allowed.');
  });
  it('keeps quiet about a refusal the open Activity panel words itself', () => {
    expect(toastFor([refused('forbidden')], true)).toBeNull();
    expect(toastFor([refused('nothing_to_revert')], true)).toBeNull();
  });
  it('still words a refusal the panel has no words for, and a rejected op, while the panel is open', () => {
    expect(toastFor([refused('odd_code')], true)).toBe('The change was not made (odd_code).');
    expect(toastFor([{ type: 'opRejected', key: 'k', code: 'conflict', node: null, retryable: false }], true)).toMatch(/Someone changed this node first/);
  });
  it('says nothing for events that are not refusals', () => {
    expect(toastFor([{ type: 'activityChanged' }], false)).toBeNull();
  });
});
