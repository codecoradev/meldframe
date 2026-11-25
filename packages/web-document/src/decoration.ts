// The Border and Shadow sections: what a typed value or a click sends. The rules (the limit of shadows, the default of a
// new one, the ranges and the colours, which the core checks and refuses with a notice) are the app layer's; a typed
// text goes to it as typed (`textAction`) and these functions turn a click into the action to send, or into nothing
// when there is nothing to send. Pure.
import type { Action, BorderPosition, Properties } from '@meldframe/protocol';
import { textAction } from './properties';

export const BORDER_POSITIONS: { value: BorderPosition; label: string }[] = [
  { value: 'inside', label: 'Inside' },
  { value: 'center', label: 'Center' },
  { value: 'outside', label: 'Outside' },
];

export type ShadowField = 'x' | 'y' | 'blur' | 'spread' | 'color';
/** The numbers of a shadow in the order of the board: offset, blur, spread. */
export const SHADOW_NUMBERS: { field: Exclude<ShadowField, 'color'>; label: string }[] = [
  { field: 'x', label: 'X' },
  { field: 'y', label: 'Y' },
  { field: 'blur', label: 'Blur' },
  { field: 'spread', label: 'Spread' },
];

const set = (p: Properties, property: Extract<Action, { type: 'setProperty' }>['property']): Action => ({ type: 'setProperty', id: p.id, property });

/** Remove the border: the one command of the width field and of an agent, width 0 (colour and position stay, so one undo brings all three back). Nothing when there is no border. */
export const removeBorderAction = (p: Properties): Action | null => (p.border.width > 0 ? textAction(p, { type: 'borderWidth' }, '0') : null);

export const borderPositionAction = (p: Properties, position: BorderPosition): Action | null => (position === p.border.position ? null : set(p, { property: 'borderPosition', value: position }));

/** One part of one shadow, as typed. */
export const shadowAction = (p: Properties, index: number, part: ShadowField, raw: string): Action => textAction(p, { type: 'shadow', index, part: { type: part } }, raw);

export const addShadowAction = (p: Properties): Action => set(p, { property: 'addShadow', value: {} });
export const removeShadowAction = (p: Properties, index: number): Action => set(p, { property: 'removeShadow', value: index });
