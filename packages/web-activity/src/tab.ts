// What the page does around the two tabs of the right card. The tab itself and the fetch on first opening are the app layer's
// (`AppState.rightTab`, `Action.activity` with `setTab`; finding T10). Pure.
import type { Event } from '@meldframe/protocol';
import { noticeFromEvents } from '@meldframe/web-document/notices';
import { messageFor } from './view';

/**
 * The toast for the events of one dispatch. A refusal the open Activity panel words itself is not said twice; any other
 * refusal (the panel has no words for it) still gets the toast.
 */
export function toastFor(events: readonly Event[], panelShows: boolean): string | null {
  return panelShows && messageFor(events)?.tone === 'warning' ? null : noticeFromEvents(events);
}
