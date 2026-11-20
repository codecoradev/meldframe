// The Fill section: what the type switch, the gradient fields and the bind picker send. The rules (what a type does to
// the fill, the order and the number of the stops, the limits, the kinds that take a gradient, and the checks of a colour, which the core makes) are the app layer's; these functions turn
// a click or a typed text into the action to send, or into nothing when there is nothing to send. Pure.
import type { Action, FillKind, Gradient, Properties } from '@meldframe/protocol';

/** The types the switch offers, in the order of the Fill board. Image is shown when the node has one, not offered. */
export const FILL_TYPES: { value: FillKind; label: string }[] = [
  { value: 'solid', label: 'Solid' },
  { value: 'linear', label: 'Linear' },
  { value: 'radial', label: 'Radial' },
  { value: 'none', label: 'None' },
];

const set = (p: Properties, property: Extract<Action, { type: 'setProperty' }>['property']): Action => ({ type: 'setProperty', id: p.id, property });

/** The type switch: nothing to send for the type the fill has already, or for the image (it cannot be picked here). */
export function fillTypeAction(p: Properties, kind: FillKind): Action | null {
  return kind === p.fillKind || kind === 'image' ? null : set(p, { property: 'fillType', value: kind });
}

const num = (raw: string): number | null => {
  const t = raw.trim();
  if (t === '') return null;
  const v = Number(t);
  return Number.isFinite(v) ? v : null;
};

/** The angle typed in degrees. */
export function angleAction(p: Properties, raw: string): Action | null {
  const v = num(raw);
  return v === null || !p.gradient || v === p.gradient.angle ? null : set(p, { property: 'gradientAngle', value: v });
}

/** The position of a stop is typed as a percentage; the file holds 0 to 1. */
export const stopPercent = (pos: number): string => String(Math.round(pos * 1000) / 10);

export function stopPosAction(p: Properties, index: number, raw: string): Action | null {
  const stop = p.gradient?.stops[index];
  const v = num(raw);
  if (!stop || v === null || v < 0 || v > 100) return null;
  const pos = Math.round(v * 10) / 1000;
  return pos === stop.pos ? null : set(p, { property: 'gradientStop', value: { index, pos, color: stop.color } });
}

export function stopColorAction(p: Properties, index: number, raw: string): Action | null {
  const stop = p.gradient?.stops[index];
  const color = raw.trim();
  // Whether it is a colour is the core's to say: a text that is not one is refused with a notice, not dropped here.
  if (!stop || color === '' || color.toLowerCase() === stop.color.toLowerCase()) return null;
  return set(p, { property: 'gradientStop', value: { index, pos: stop.pos, color } });
}

export const addStopAction = (p: Properties): Action => set(p, { property: 'addGradientStop', value: {} });
export const removeStopAction = (p: Properties, index: number): Action => set(p, { property: 'removeGradientStop', value: index });

/** The CSS of the preview bar: the stops along a line to the right, whatever the angle or the kind. */
export function gradientBar(g: Gradient): string {
  return `linear-gradient(90deg, ${g.stops.map((s) => `${s.color} ${Math.round(s.pos * 1000) / 10}%`).join(', ')})`;
}
