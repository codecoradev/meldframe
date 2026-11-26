// The properties panel: what an edit becomes. A text that was typed goes to the app layer as it is, with the name of the
// field (`typed`): the parse, the range of the field and the decision that nothing changes are the app
// layer's (`crates/core/meldframe-app-parts/src/typed.rs`). The clicks that choose a value are turned into their action
// here. Pure.
import type { Action, BindField, FieldBinding, Properties, TextAlign, TextField } from '@meldframe/protocol';

/** The fields that are named by their type alone (the others, a shadow part, a stop, a side, also say which one). */
export type PlainField = Exclude<TextField, { index: number } | { side: unknown }>;

/** The text typed in a field of the panel, as typed: the app layer parses it, holds it to the range of the field and sends nothing when there is nothing to change. */
export const textAction = (p: Properties, field: TextField, text: string): Action => ({ type: 'typed', entry: { type: 'property', id: p.id, field, text } });

/** The clip switch of a frame: nothing to send when it already is so. */
export function clipAction(p: Properties, clip: boolean): Action | null {
  return clip === p.clip ? null : { type: 'setProperty', id: p.id, property: { property: 'clip', value: clip } };
}

/**
 * The weights of a text, as the model takes them: 100 to 900 in steps of 100 (`fontWeight`, `crates/core/meldframe-check`
 * refuses any other number as E_INVALID_FONT_WEIGHT), named as the CSS weights are. The value is a string, as a select reports it.
 */
export const FONT_WEIGHTS: { value: string; label: string }[] = [
  { value: '100', label: 'Thin' },
  { value: '200', label: 'Extra Light' },
  { value: '300', label: 'Light' },
  { value: '400', label: 'Regular' },
  { value: '500', label: 'Medium' },
  { value: '600', label: 'Semi Bold' },
  { value: '700', label: 'Bold' },
  { value: '800', label: 'Extra Bold' },
  { value: '900', label: 'Black' },
];

/** The alignments of a text in its box (`textAlign`): left, center or right; the model has no justify. */
export const TEXT_ALIGNS: { value: TextAlign; label: string; icon: 'text-left' | 'text-center' | 'text-right' }[] = [
  { value: 'left', label: 'Align left', icon: 'text-left' },
  { value: 'center', label: 'Align center', icon: 'text-center' },
  { value: 'right', label: 'Align right', icon: 'text-right' },
];

/** The weight chosen in the select of a text: nothing to send for the weight it has, for a value the model does not take, or for a node that is no text. */
export function fontWeightAction(p: Properties, raw: string): Action | null {
  const w = FONT_WEIGHTS.find((x) => x.value === raw);
  return w && p.fontWeight !== undefined && Number(w.value) !== p.fontWeight ? { type: 'setProperty', id: p.id, property: { property: 'fontWeight', value: Number(w.value) } } : null;
}

/** The alignment chosen for a text: nothing to send for the one it has or for a node that is no text. */
export const textAlignAction = (p: Properties, align: TextAlign): Action | null =>
  p.textAlign !== undefined && align !== p.textAlign ? { type: 'setProperty', id: p.id, property: { property: 'textAlign', value: align } } : null;

/** An opacity of 0 to 1 as the percentage its field shows. */
export const percent = (opacity: number): string => String(Math.round(opacity * 100));

/** The variable a field is bound to, for its chip. */
export const bindingOf = (p: Properties, field: BindField): FieldBinding | undefined => p.bindings.find((b) => b.field === field);

/** Let go of a bound field (it keeps the value it shows); nothing to do for a field that is free. */
export function unbindAction(p: Properties, field: BindField): Action | null {
  return bindingOf(p, field) ? { type: 'bindProperty', id: p.id, field, variable: null } : null;
}
