// The layout fields of the inspector: what a typed value or a click sends, and the words of a note. The rules (what
// each field does, which settings have no effect where the node is) are the app layer's; these functions turn input into
// the action to send, or into nothing when there is nothing to send. Pure.
import type { Action, Align, Axis, Justify, LayoutMode, LayoutNote, Properties, Side, SizeMode } from '@meldframe/protocol';

type Prop = Extract<Action, { type: 'setProperty' }>['property'];
const set = (p: Properties, property: Prop): Action => ({ type: 'setProperty', id: p.id, property });

export const LAYOUT_MODES: { value: LayoutMode; label: string }[] = [
  { value: 'none', label: 'Free' },
  { value: 'row', label: 'Row' },
  { value: 'column', label: 'Column' },
];
export const ALIGNS: { value: Align; label: string }[] = [
  { value: 'start', label: 'Start' },
  { value: 'center', label: 'Center' },
  { value: 'end', label: 'End' },
];
export const JUSTIFIES: { value: Justify; label: string }[] = [...ALIGNS, { value: 'between', label: 'Between' }];
export const SIZE_MODES: { value: SizeMode; label: string }[] = [
  { value: 'fixed', label: 'Fixed' },
  { value: 'hug', label: 'Hug' },
  { value: 'fill', label: 'Fill' },
];
export const SIDES: { side: Side; label: string }[] = [
  { side: 'top', label: 'Top' },
  { side: 'right', label: 'Right' },
  { side: 'bottom', label: 'Bottom' },
  { side: 'left', label: 'Left' },
];

/** A length typed in px: a number of 0 or more. */
const length = (raw: string): number | null => {
  const t = raw.trim();
  if (t === '') return null;
  const v = Number(t);
  return Number.isFinite(v) && v >= 0 ? v : null;
};

export const layoutAction = (p: Properties, mode: LayoutMode): Action | null => (mode === p.layout ? null : set(p, { property: 'layout', value: mode }));
export const alignAction = (p: Properties, a: Align): Action | null => (a === p.align ? null : set(p, { property: 'align', value: a }));
export const justifyAction = (p: Properties, j: Justify): Action | null => (j === p.justify ? null : set(p, { property: 'justify', value: j }));
export const sizingAction = (p: Properties, axis: Axis, mode: SizeMode): Action | null => ((axis === 'width' ? p.sw : p.sh) === mode ? null : set(p, { property: 'sizing', value: { axis, mode } }));
export const absoluteAction = (p: Properties, v: boolean): Action | null => (v === p.absolute ? null : set(p, { property: 'absolute', value: v }));

export function gapAction(p: Properties, raw: string): Action | null {
  const v = length(raw);
  return v === null || v === p.gap ? null : set(p, { property: 'gap', value: v });
}
export function paddingSideAction(p: Properties, side: Side, raw: string): Action | null {
  const v = length(raw);
  return v === null || v === p.padding[side] ? null : set(p, { property: 'padding', value: { side, value: v } });
}
/** The uniform field: all four sides take the value, even when some already have it. */
export function paddingAllAction(p: Properties, raw: string): Action | null {
  const v = length(raw);
  return v === null || v === p.paddingUniform ? null : set(p, { property: 'paddingAll', value: v });
}
export function marginSideAction(p: Properties, side: Side, raw: string): Action | null {
  const v = length(raw);
  return v === null || v === p.margin[side] ? null : set(p, { property: 'margin', value: { side, value: v } });
}

/** The Margin section is there in an auto layout parent, and wherever the node has a margin or is absolute: a note says
 *  they have no effect where the node is, and the fields that clear them must not be hidden by it. */
export const showPlacement = (p: Properties): boolean => p.inLayout || p.absolute || Object.values(p.margin).some((v) => v > 0);

const axisWord = (a: Axis): string => (a === 'width' ? 'width' : 'height');

/** What a note says to the person. */
export function noteText(n: LayoutNote): string {
  switch (n.note) {
    case 'fillUnderHug':
      return `Fill ${axisWord(n.axis)} needs a parent with auto layout that does not hug: the export keeps this ${axisWord(n.axis)}.`;
    case 'absoluteFillIgnored':
      return `Fill ${axisWord(n.axis)} has no effect on an absolute node: it keeps its own ${axisWord(n.axis)}.`;
    case 'hugIgnored':
      return `Hug ${axisWord(n.axis)} only applies to text and to a frame with auto layout: this ${axisWord(n.axis)} stays fixed.`;
    case 'marginIgnored':
      return 'The margin takes no room here: this node is not in the flow of an auto layout parent (there is none, or the node is absolute), so it sits where its position says.';
    case 'absoluteIgnored':
      return 'Absolute has no effect here: the parent has no auto layout, so every child sits at its own position.';
  }
}
