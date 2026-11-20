import { describe, expect, it } from 'vitest';
import type { AppState } from '@meldframe/protocol';
import { bannerFor, connectionLabel } from './connection';

type C = AppState['connection'];
const live: C = { type: 'live' };

describe('the words of the connection', () => {
  it('names each state for the status strip', () => {
    expect(connectionLabel(live)).toBe('Connected');
    expect(connectionLabel({ type: 'connecting' })).toBe('Connecting');
    expect(connectionLabel({ type: 'reconnecting', attempt: 2 })).toBe('Reconnecting');
    expect(connectionLabel({ type: 'offline' })).toBe('Offline');
    expect(connectionLabel({ type: 'halted' })).toBe('Out of sync');
  });
});

describe('the connection banner', () => {
  it('shows nothing while the page is connecting or live', () => {
    expect(bannerFor({ type: 'connecting' }, 0)).toBeNull();
    expect(bannerFor(live, 3)).toBeNull();
  });

  it('says it is reconnecting, with the attempt, and how many changes will be sent', () => {
    const none = bannerFor({ type: 'reconnecting', attempt: 2 }, 0);
    expect(none).toMatchObject({ tone: 'warning', text: 'Reconnecting to the server. Attempt 2.' });
    expect(none?.action).toBeUndefined();
    expect(bannerFor({ type: 'reconnecting', attempt: 1 }, 1)?.text).toBe('Reconnecting to the server. 1 change will be sent when it is back; keep this tab open. Attempt 1.');
    expect(bannerFor({ type: 'reconnecting', attempt: 3 }, 4)?.text).toBe('Reconnecting to the server. 4 changes will be sent when it is back; keep this tab open. Attempt 3.');
  });

  it('offline counts the changes that wait and says they are kept only while the tab is open, and offers Retry now', () => {
    expect(bannerFor({ type: 'offline' }, 0)).toMatchObject({ tone: 'warning', text: "Offline. Can't reach the server.", action: { id: 'retry', label: 'Retry now' } });
    expect(bannerFor({ type: 'offline' }, 1)?.text).toBe("Offline. Can't reach the server. 1 change is waiting to be sent; keep this tab open.");
    expect(bannerFor({ type: 'offline' }, 3)?.text).toBe("Offline. Can't reach the server. 3 changes are waiting to be sent; keep this tab open.");
  });

  it('a page out of step with the server is an alert with Reload', () => {
    expect(bannerFor({ type: 'halted' }, 2)).toMatchObject({ tone: 'danger', text: 'This page and the server no longer agree about the file.', action: { id: 'reload', label: 'Reload' } });
  });

  it('never promises more than the page holds: the waiting changes are in this tab only', () => {
    for (const c of [{ type: 'offline' }, { type: 'reconnecting', attempt: 1 }] as const) {
      expect(bannerFor(c, 2)!.text).not.toMatch(/nothing was lost|saved|safe/i);
      expect(bannerFor(c, 2)!.text).toMatch(/keep this tab open/);
    }
  });

  it('the banner has a stable id per state, so a change of attempt updates the same banner', () => {
    expect(bannerFor({ type: 'reconnecting', attempt: 1 }, 0)?.id).toBe(bannerFor({ type: 'reconnecting', attempt: 2 }, 0)?.id);
    expect(bannerFor({ type: 'offline' }, 0)?.id).not.toBe(bannerFor({ type: 'halted' }, 0)?.id);
  });
});
