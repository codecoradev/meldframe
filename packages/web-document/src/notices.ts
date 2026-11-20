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
  quota_storage: 'The workspace storage is full. Your files are unchanged; edits that add data are paused until space is freed.',
  rate_limited: 'Too fast: wait a moment and try again.',
  outbox_full: 'Too many changes are waiting for the server. Wait until it catches up.',
};

// Refusals made on this page, before anything is sent: the Variables panel and the checks of the document.
const LOCAL: Record<string, string> = {
  invalid_value: 'That is not a number.',
  not_bindable: 'That field cannot take a variable here.',
  invalid: 'That value is not allowed here.',
  limit: 'That is the most there can be.',
  min_stops: 'A gradient keeps at least two stops.',
  no_gradient: 'This node has no gradient.',
  no_image: 'This node has no image to show.',
  gradient_on_text: 'Text is painted with a colour: put the gradient on a frame or a shape behind it.',
  not_selected: 'Select that node first.',
  E_INVALID_COLOR: 'That is not a colour. Use a hex colour.',
  E_NOT_FINITE: 'That number is not valid.',
  E_OUT_OF_RANGE: 'That number is out of range.',
  E_TYPE_MISMATCH: 'That variable has the wrong type for this field.',
  E_ALIAS_CYCLE: 'An alias cannot lead back to itself.',
  E_ALIAS_TOO_DEEP: 'That chain of aliases is too long.',
  E_VARIABLE_NOT_FOUND: 'That variable no longer exists.',
  E_MODE_NOT_FOUND: 'That mode no longer exists.',
  E_COLLECTION_NOT_FOUND: 'That collection no longer exists.',
};

/** The message for a refusal, null for any other event and for a rejection that the app sends again by itself. */
export function noticeFor(e: Event): string | null {
  if (e.type === 'opRejected' && e.retryable) return null;
  if (e.type !== 'refused' && e.type !== 'opRejected') return null;
  // A refusal of the app was decided here; only a rejection of an op comes from the server.
  if (e.type === 'refused') return WORDS[e.code] ?? LOCAL[e.code] ?? `The change was not made (${e.code}).`;
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
