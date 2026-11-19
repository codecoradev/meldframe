// What a refusal says to the person. Pure.
import type { Event } from '@meldframe/protocol';

const WORDS: Record<string, string> = {
  forbidden: 'This change is not allowed.',
  approval_required: "This delete needs the owner's approval and was not done.",
  approval_denied: 'The owner said no. The change was not made.',
  approval_expired: 'The approval ran out. The change was not made.',
  approval_limit: 'Too many approvals are open. Try again later.',
  conflict: 'Someone changed this node first. Look at the new version and try again.',
  not_found: 'That node no longer exists.',
  stale_client: 'This page is out of date. Reload it.',
  rate_limited: 'Too fast: wait a moment and try again.',
  outbox_full: 'Too many changes are waiting for the server. Wait until it catches up.',
};

/** The message for a refusal, null for any other event and for a rejection that the app sends again by itself. */
export function noticeFor(e: Event): string | null {
  if (e.type === 'opRejected' && e.retryable) return null;
  if (e.type !== 'refused' && e.type !== 'opRejected') return null;
  return WORDS[e.code] ?? `The server refused the change (${e.code}).`;
}

/** The message for the last refusal among the events of one dispatch, or null. */
export function noticeFromEvents(events: readonly Event[]): string | null {
  for (let i = events.length - 1; i >= 0; i--) {
    const text = noticeFor(events[i]!);
    if (text) return text;
  }
  return null;
}
