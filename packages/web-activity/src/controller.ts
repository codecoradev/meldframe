// What the panel remembers: view state only, which lanes the person opened or folded. The question being asked (revert a batch,
// restore a checkpoint), the tab and whether a checkpoint was made are the app layer's (`AppState.confirm`, `right_tab`,
// `Event.checkpointMade`; finding T10 of the thin UI audit); the controls send `actions.ts`.

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
