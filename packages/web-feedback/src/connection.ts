// What the page says about its connection: the word in the status strip, and the banner over the canvas while the
// connection is not live. The state is the app layer's (`state.connection`, `state.pending`); this only words it. Pure.
import type { AppState } from '@meldframe/protocol';

type Connection = AppState['connection'];

/** The word of the status strip. */
export function connectionLabel(c: Connection): string {
  return { live: 'Connected', connecting: 'Connecting', reconnecting: 'Reconnecting', offline: 'Offline', halted: 'Out of sync' }[c.type];
}

/** A banner over the canvas. `action` names what its button does; the page decides how (`retry` dispatches `reconnect`, `reload` reloads). */
export interface BannerView {
  id: string;
  tone: 'warning' | 'danger';
  text: string;
  action?: { id: 'retry' | 'reload'; label: string };
}

const changes = (n: number) => `${n} ${n === 1 ? 'change' : 'changes'}`;

/** The banner for a connection that is not live, or null while it is connecting or live. `pending` is the unconfirmed ops. */
export function bannerFor(c: Connection, pending: number): BannerView | null {
  switch (c.type) {
    case 'reconnecting':
      return {
        id: 'connection',
        tone: 'warning',
        text: pending > 0 ? `Reconnecting to the server. ${changes(pending)} will be sent when it is back; keep this tab open. Attempt ${c.attempt}.` : `Reconnecting to the server. Attempt ${c.attempt}.`,
      };
    case 'offline':
      return {
        id: 'connection',
        tone: 'warning',
        text: pending > 0 ? `Offline. Can't reach the server. ${changes(pending)} ${pending === 1 ? 'is' : 'are'} waiting to be sent; keep this tab open.` : "Offline. Can't reach the server.",
        action: { id: 'retry', label: 'Retry now' },
      };
    case 'halted':
      return { id: 'halted', tone: 'danger', text: 'This page and the server no longer agree about the file.', action: { id: 'reload', label: 'Reload' } };
    default:
      return null;
  }
}
