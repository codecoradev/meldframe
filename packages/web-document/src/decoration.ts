// The Border and Shadow sections: what a typed value or a click sends. The rules (the limit of shadows, the default of a
// new one, the ranges and the colours, which the core checks and refuses with a notice) are the app layer's; these
// functions turn input into the action to send, or into nothing when there is nothing to send. Pure.
import type { Action, BorderPosition, Properties, Shadow } from '@meldframe/protocol';

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

const num = (raw: string): number | null => {
  const t = raw.trim();
  if (t === '') return null;
  const v = Number(t);
  return Number.isFinite(v) ? v : null;
};

export function borderWidthAction(p: Properties, raw: string): Action | null {
  const v = num(raw);
  return v === null || v < 0 || v === p.border.width ? null : set(p, { property: 'borderWidth', value: v });
}

export function borderColorAction(p: Properties, raw: string): Action | null {
  const v = raw.trim();
  // Whether it is a colour is the core's to say; an empty field sends nothing.
  return v !== '' && v.toLowerCase() !== p.border.color.toLowerCase() ? set(p, { property: 'borderColor', value: v }) : null;
}

export const borderPositionAction = (p: Properties, position: BorderPosition): Action | null => (position === p.border.position ? null : set(p, { property: 'borderPosition', value: position }));

/** One field of one shadow: the numbers as typed (blur not below 0), the colour as a #hex. */
export function shadowAction(p: Properties, index: number, field: ShadowField, raw: string): Action | null {
  const current = p.shadows[index];
  if (!current) return null;
  let next: Shadow;
  if (field === 'color') {
    const v = raw.trim();
    if (v === '' || v.toLowerCase() === current.color.toLowerCase()) return null;
    next = { ...current, color: v };
  } else {
    const v = num(raw);
    if (v === null || v === current[field] || (field === 'blur' && v < 0)) return null;
    next = { ...current, [field]: v };
  }
  return set(p, { property: 'setShadow', value: { index, shadow: next } });
}

export const addShadowAction = (p: Properties): Action => set(p, { property: 'addShadow', value: {} });
export const removeShadowAction = (p: Properties, index: number): Action => set(p, { property: 'removeShadow', value: index });
