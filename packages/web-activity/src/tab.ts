// The two tabs of the right card and what the page does around them. Pure.
import type { Action, Event } from '@meldframe/protocol';
import { noticeFromEvents } from '@meldframe/web-document/notices';
import { messageFor } from './view';

export type RightTab = 'design' | 'activity';

/**
 * Selecting a tab. Only a change to Activity asks the app layer for the newest batches (heal rule (c), a person's
 * action); the tab that is already selected asks for nothing, so a second click or a key on it does not fetch again.
 */
export function tabChange(current: RightTab, next: RightTab): { tab: RightTab; actions: Action[] } {
  return { tab: next, actions: next === 'activity' && current !== 'activity' ? [{ type: 'openActivity' }] : [] };
}

/**
 * The toast for the events of one dispatch. A refusal the open Activity panel words itself is not said twice; any other
 * refusal (the panel has no words for it) still gets the toast.
 */
export function toastFor(events: readonly Event[], panelShows: boolean): string | null {
  return panelShows && messageFor(events)?.tone === 'warning' ? null : noticeFromEvents(events);
}
