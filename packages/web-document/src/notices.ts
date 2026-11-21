// What a refusal says to the person. Pure.
import type { Event } from '@meldframe/protocol';

/** The rule of the name of a variable or a collection (`valid_name` in meldframe-variables), as the field and the refusal both say it. */
export const NAME_RULE_WORDS = 'Use lowercase letters, digits and single hyphens, with / between the parts, such as color/brand-500.';

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
  outbox_full: 'Too many changes are waiting for the server, so nothing was changed. Wait until it catches up, then try again.',
  // The Variables manage view. Each of these can come back from the app layer before anything is sent, or from the server
  // when an op lost a race with another actor's, so the words fit both. The limits are those of `Limits` in meldframe-check.
  E_INVALID_NAME: NAME_RULE_WORDS,
  E_DUPLICATE_NAME: 'A variable in this collection already has that name.',
  // The core uses these three for nodes, texts, shadows and gradient stops as well as for variables, so the words here name
  // none of them; the manage view words them for what it sent (web-panels manage.ts).
  E_TOO_LONG: 'That text is longer than this field allows. Shorten it.',
  E_TOO_MANY_ENTRIES: 'That would go over a limit: there are already as many as the file allows. Remove one first.',
  E_DEFAULT_MODE: 'The first mode is the default one and stays. Remove another mode.',
  E_VARIABLE_IN_USE: 'Something still uses this variable, so it was not removed. Remove it again and choose Detach to turn each use into its value.',
  E_COLLECTION_NOT_EMPTY: 'This collection still has variables, so it was not removed. Remove it again and choose Detach to remove them with it.',
  E_VARIABLE_NOT_FOUND: 'That variable no longer exists.',
  E_MODE_NOT_FOUND: 'That mode no longer exists.',
  E_COLLECTION_NOT_FOUND: 'That collection no longer exists.',
  E_INCOMPLETE_VALUES: 'The modes of this collection changed at the same moment, so the variable was not added. Try again.',
  E_INVALID_MODES: 'That mode could not be added to this collection.',
  E_DUPLICATE_ID: 'Something with the same id exists already. Try again.',
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
