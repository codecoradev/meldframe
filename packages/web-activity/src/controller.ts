// What the panel remembers: view state only, which lanes the person opened or folded. The question being asked (revert a batch,
// restore a checkpoint), the tab and whether a checkpoint was made are the app layer's (`AppState.confirm`, `right_tab`,
// `Event.checkpointMade`; finding T10 of the thin UI audit); the controls send `actions.ts`. The two small rules that the Svelte
// and the React copy of the panel share (what the last request was about, what the events of a dispatch say) are here, tested in Node.
import type { Action, Event } from '@meldframe/protocol';
import { checkpointMade, messageFor, type Message } from './view';

export interface Ui {
  /** The person's choice for a lane, by actor: open (true) or folded (false). A lane without a choice follows the default. */
  lanes: Readonly<Record<string, boolean>>;
}

export const start: Ui = { lanes: {} };

/** Enter in the field makes the checkpoint; Enter that confirms an input method's text, or Shift+Enter, does not. */
export const isSubmitKey = (e: { key: string; isComposing: boolean; shiftKey: boolean }): boolean => e.key === 'Enter' && !e.isComposing && !e.shiftKey;

/** Open or folded: the person's choice for the actor if there is one, else only the first lane is open. */
export const laneOpen = (ui: Ui, actor: string, index: number): boolean => ui.lanes[actor] ?? index === 0;

export const toggleLane = (ui: Ui, actor: string, open: boolean): Ui => ({ lanes: { ...ui.lanes, [actor]: !open } });

/** What the person asked for last: a checkpoint's request (Create, a confirmed revert or restore) or anything else. */
export type About = 'checkpoint' | undefined;

/**
 * What the last request was about after `a` was sent, which decides the words of a refusal: `forbidden` means one thing for
 * a review and another for a checkpoint. A review goes back to the words of a review; other actions leave it as it was.
 */
export function aboutAfter(about: About, a: Action): About {
  if (a.type === 'createCheckpoint' || (a.type === 'activity' && (a.op.type === 'confirmRevert' || a.op.type === 'confirmRestore'))) return 'checkpoint';
  if (a.type === 'setBatchReview') return undefined;
  return about;
}

/**
 * What the events of one dispatch do to an open panel: a checkpoint this page asked for was made (the field empties, the line
 * says so), or the line says what a refusal, a revert or a restore was (`messageFor`), or nothing.
 */
export function eventsSay(events: readonly Event[], about: About): { made: boolean; message: Message | null } {
  if (events.some((e) => e.type === 'checkpointMade')) return { made: true, message: checkpointMade };
  return { made: false, message: messageFor(events, about) };
}
