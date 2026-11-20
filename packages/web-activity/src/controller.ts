// What the panel remembers (view state only) and what each control does, as pure steps: a step is the next view state and
// the actions to dispatch. The Svelte files only call these, so a sequence of clicks is a test and not a promise. Which
// batch may be reverted is the app layer's (`revertable`, `inEffect`); this holds the confirmation the person is in.
import type { Action, ActivityState, BatchRef, BatchRow, Checkpoint } from '@meldframe/protocol';
import { refOf, restore, revert, show as showAction } from './actions';

/** The one question the person is being asked: revert a batch, or restore a checkpoint. */
export type Confirm = ({ kind: 'revert' } & BatchRef) | { kind: 'restore'; id: string };

export interface Ui {
  confirm: Confirm | null;
  /** The person's choice for a lane, by actor: open (true) or folded (false). A lane without a choice follows the default. */
  lanes: Readonly<Record<string, boolean>>;
}

export interface Step {
  ui: Ui;
  actions: Action[];
}

export const start: Ui = { confirm: null, lanes: {} };

const same = (r: BatchRef, o: BatchRef | null) => o !== null && r.actor === o.actor && r.keyPrefix === o.keyPrefix;

/**
 * Asking to revert selects the batch first, so the app layer works out whether a revert would still change anything
 * (it does that for the selected row only), and opens the confirmation.
 */
export const askRevert = (ui: Ui, r: BatchRow): Step => ({ ui: { ...ui, confirm: { kind: 'revert', ...refOf(r) } }, actions: [showAction(r)] });

/**
 * The revert goes out only while the confirmation is for this batch and it is still the selected one (the answer on
 * the screen was worked out for the selected row), and only once: the confirmation closes with it.
 */
export function confirmRevert(ui: Ui, r: BatchRow, selected: BatchRef | null): Step {
  const c = ui.confirm;
  if (c?.kind !== 'revert' || !same(c, refOf(r)) || !same(c, selected)) return { ui, actions: [] };
  return { ui: { ...ui, confirm: null }, actions: [revert(r)] };
}

export const askRestore = (ui: Ui, id: string): Step => ({ ui: { ...ui, confirm: { kind: 'restore', id } }, actions: [] });

export function confirmRestore(ui: Ui, id: string): Step {
  if (ui.confirm?.kind !== 'restore' || ui.confirm.id !== id) return { ui, actions: [] };
  return { ui: { ...ui, confirm: null }, actions: [restore(id)] };
}

/** Cancel, Close and Esc. */
export const cancel = (ui: Ui): Step => ({ ui: { ...ui, confirm: null }, actions: [] });

/** Show selects the batch's nodes; a revert confirmation for another batch closes, because the selection moves away from it. */
export function show(ui: Ui, r: BatchRow): Step {
  const c = ui.confirm;
  const keep = c === null || c.kind !== 'revert' || same(c, refOf(r));
  return { ui: keep ? ui : { ...ui, confirm: null }, actions: [showAction(r)] };
}

/** Open or folded: the person's choice for the actor if there is one, else only the first lane is open. */
export const laneOpen = (ui: Ui, actor: string, index: number): boolean => ui.lanes[actor] ?? index === 0;

export const toggleLane = (ui: Ui, actor: string, open: boolean): Step => ({ ui: { ...ui, lanes: { ...ui.lanes, [actor]: !open } }, actions: [] });

/**
 * The view state after the app layer's state moved: a revert confirmation ends when its row is gone or is no longer the
 * selected one, a restore confirmation when its checkpoint is gone. Returns `ui` itself when nothing changed.
 */
export function settle(ui: Ui, a: ActivityState, checkpoints: readonly Checkpoint[]): Ui {
  const c = ui.confirm;
  if (c === null) return ui;
  const holds = c.kind === 'restore' ? checkpoints.some((k) => k.id === c.id) : same(c, a.selected) && a.rows.some((r) => same(c, r));
  return holds ? ui : { ...ui, confirm: null };
}
