// The properties panel: what an edited value becomes. A value that is unchanged or not valid makes no action (and so
// no undo step); the field then shows the document's value again. Pure.
import type { Action, BindField, FieldBinding, NodeKind, Properties } from '@meldframe/protocol';
import { MIN_SIZE } from './edit';

const num = (raw: string): number | null => {
  const t = raw.trim();
  if (t === '') return null;
  const v = Number(t);
  return Number.isFinite(v) ? v : null;
};
const HEX = /^#([0-9a-f]{3}|[0-9a-f]{6}|[0-9a-f]{8})$/i;

export type PropertyField = 'name' | 'fill' | 'radius' | 'fontSize' | 'opacity';
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
      // A #hex colour, or the word none for no fill.
      return (HEX.test(v) || v === 'none') && v.toLowerCase() !== p.fill.toLowerCase() ? set({ property: 'fill', value: v }) : null;
    }
    case 'radius': {
      const v = num(raw);
      return v === null || v < 0 || v === p.radius ? null : set({ property: 'radius', value: v });
    }
    case 'opacity': {
      // Typed as a percentage; the document holds 0 to 1 (three decimals keep a typed 33.3 as it is).
      const v = num(raw);
      if (v === null || v < 0 || v > 100) return null;
      const value = Math.round(v * 10) / 1000;
      return value === p.opacity ? null : set({ property: 'opacity', value });
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

/** The clip switch of a frame: nothing to send when it already is so. */
export function clipAction(p: Properties, clip: boolean): Action | null {
  return clip === p.clip ? null : { type: 'setProperty', id: p.id, property: { property: 'clip', value: clip } };
}

/** An opacity of 0 to 1 as the percentage its field shows. */
export const percent = (opacity: number): string => String(Math.round(opacity * 100));

/** The variable a field is bound to, for its chip. */
export const bindingOf = (p: Properties, field: BindField): FieldBinding | undefined => p.bindings.find((b) => b.field === field);

/** Let go of a bound field (it keeps the value it shows); nothing to do for a field that is free. */
export function unbindAction(p: Properties, field: BindField): Action | null {
  return bindingOf(p, field) ? { type: 'bindProperty', id: p.id, field, variable: null } : null;
}

/** Which of the optional fields a kind of node has: the canvas draws no corners on text and an ellipse, a frame alone clips. */
export function fieldsOf(kind: NodeKind): { radius: boolean; clip: boolean; fontSize: boolean } {
  return { radius: kind === 'frame' || kind === 'rect' || kind === 'shader', clip: kind === 'frame', fontSize: kind === 'text' };
}
