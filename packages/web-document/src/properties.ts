// The properties panel: what an edited value becomes. A value that is unchanged or not valid makes no action (and so
// no undo step); the field then shows the document's value again. Pure.
import type { Action, Properties } from '@meldframe/protocol';
import { MIN_SIZE } from './edit';

const num = (raw: string): number | null => {
  const t = raw.trim();
  if (t === '') return null;
  const v = Number(t);
  return Number.isFinite(v) ? v : null;
};
const HEX = /^#([0-9a-f]{3}|[0-9a-f]{6}|[0-9a-f]{8})$/i;

export type PropertyField = 'name' | 'fill' | 'radius' | 'fontSize';
export type BoxField = 'x' | 'y' | 'w' | 'h';

export function propertyAction(p: Properties, field: PropertyField, raw: string): Action | null {
  const set = (property: Extract<Action, { type: 'setProperty' }>['property']): Action => ({ type: 'setProperty', id: p.id, property });
  switch (field) {
    case 'name': {
      const v = raw.trim();
      return v === '' || v === p.name ? null : set({ property: 'name', value: v });
    }
    case 'fill': {
      const v = raw.trim();
      return HEX.test(v) && v.toLowerCase() !== p.fill.toLowerCase() ? set({ property: 'fill', value: v }) : null;
    }
    case 'radius': {
      const v = num(raw);
      return v === null || v < 0 || v === p.radius ? null : set({ property: 'radius', value: v });
    }
    case 'fontSize': {
      const v = num(raw);
      return v === null || v <= 0 || v === p.fontSize ? null : set({ property: 'fontSize', value: v });
    }
  }
}

/** x and y move the node (the other coordinate stays); w and h resize it (the origin and the other side stay). */
export function boxAction(p: Properties, field: BoxField, raw: string): Action | null {
  const v = num(raw);
  if (v === null || v === p[field]) return null;
  if (field === 'x') return { type: 'move', id: p.id, x: v, y: p.y };
  if (field === 'y') return { type: 'move', id: p.id, x: p.x, y: v };
  if (v < MIN_SIZE) return null;
  return field === 'w' ? { type: 'resize', id: p.id, w: v, h: p.h } : { type: 'resize', id: p.id, w: p.w, h: v };
}
