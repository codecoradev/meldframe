import type { Properties } from '@meldframe/protocol';
import { describe, expect, it } from 'vitest';
import { bindingOf, clipAction, FONT_WEIGHTS, fontWeightAction, percent, TEXT_ALIGNS, textAction, textAlignAction, unbindAction } from './properties';

const p: Properties = { id: 'n', name: 'Box', kind: 'rect', x: 10, y: 20, w: 100, h: 50, fill: '#112233', fillKind: 'solid', gradientAllowed: true, imageAllowed: true, minStops: 2, maxStops: 8, border: { width: 0, color: '#000000', position: 'inside' }, shadows: [], maxShadows: 4, radius: 4, layout: 'none', gap: 0, padding: { top: 0, right: 0, bottom: 0, left: 0 }, margin: { top: 0, right: 0, bottom: 0, left: 0 }, sw: 'fixed', sh: 'fixed', align: 'start', justify: 'start', absolute: false, inLayout: false, positionLocked: false, notes: [], sections: { radius: true, clip: false, fontSize: false, layout: false, placement: false }, text: '', fontSize: 16, opacity: 1, clip: true, bound: [], bindings: [] };
describe('textAction', () => {
  // The parse, the range and the no-op rules of every typed field are the app layer's (property_text.rs): the text goes as typed.
  it('sends what was typed, untrimmed, with the name of the field and the id of the node', () => {
    expect(textAction(p, { type: 'opacity' }, ' 33.3 ')).toEqual({ type: 'typed', entry: { type: 'property', id: 'n', field: { type: 'opacity' }, text: ' 33.3 ' } });
    expect(textAction(p, { type: 'shadow', index: 1, part: { type: 'blur' } }, '4')).toEqual({ type: 'typed', entry: { type: 'property', id: 'n', field: { type: 'shadow', index: 1, part: { type: 'blur' } }, text: '4' } });
  });
  it('sends a text that is empty, unchanged or not valid all the same: the app layer decides', () => {
    for (const text of ['', '   ', 'abc', '-1', '#112233']) expect(textAction(p, { type: 'fill' }, text)).toMatchObject({ type: 'typed', entry: { type: 'property', text } });
  });
});

describe('opacity', () => {
  it('shows a stored 0 to 1 as a rounded percentage', () => {
    expect(percent(1)).toBe('100');
    expect(percent(0.456)).toBe('46');
    expect(percent(0)).toBe('0');
  });
});

describe('clipAction', () => {
  it('sets the other value and does nothing for the same one', () => {
    expect(clipAction(p, false)).toEqual({ type: 'setProperty', id: 'n', property: { property: 'clip', value: false } });
    expect(clipAction(p, true)).toBeNull();
  });
});

describe('the weight and the alignment of a text', () => {
  const t: Properties = { ...p, kind: 'text', fontWeight: 400, textAlign: 'left' };
  it('offers the nine weights the model takes, 100 to 900 in steps of 100, and the three alignments it takes', () => {
    expect(FONT_WEIGHTS).toEqual([
      { value: '100', label: 'Thin' }, { value: '200', label: 'Extra Light' }, { value: '300', label: 'Light' },
      { value: '400', label: 'Regular' }, { value: '500', label: 'Medium' }, { value: '600', label: 'Semi Bold' },
      { value: '700', label: 'Bold' }, { value: '800', label: 'Extra Bold' }, { value: '900', label: 'Black' },
    ]);
    expect(TEXT_ALIGNS.map((a) => a.value)).toEqual(['left', 'center', 'right']);
  });
  it('sets the weight chosen as the one setProperty the app layer turns into an updateNode patch', () => {
    expect(fontWeightAction(t, '700')).toEqual({ type: 'setProperty', id: 'n', property: { property: 'fontWeight', value: 700 } });
  });
  it('sends nothing for the weight it has, for a weight the model does not take, or for a node that is no text', () => {
    expect(fontWeightAction(t, '400')).toBeNull();
    for (const raw of ['', 'bold', '450', '0', '1000', '-100']) expect(fontWeightAction(t, raw), raw).toBeNull();
    expect(fontWeightAction(p, '700')).toBeNull();
  });
  it('sets the alignment chosen and sends nothing for the one it has or for a node that is no text', () => {
    expect(textAlignAction(t, 'center')).toEqual({ type: 'setProperty', id: 'n', property: { property: 'textAlign', value: 'center' } });
    expect(textAlignAction(t, 'left')).toBeNull();
    expect(textAlignAction(p, 'center')).toBeNull();
  });
});

describe('bindings', () => {
  const bound: Properties = { ...p, bound: ['fill'], bindings: [{ field: 'fill', variable: 'v1', name: 'color/brand', missing: false }] };
  it('finds the variable a field holds', () => {
    expect(bindingOf(bound, 'fill')?.name).toBe('color/brand');
    expect(bindingOf(bound, 'radius')).toBeUndefined();
  });
  it('lets go of a bound field and has nothing to do for a free one', () => {
    expect(unbindAction(bound, 'fill')).toEqual({ type: 'bindProperty', id: 'n', field: 'fill', variable: null });
    expect(unbindAction(bound, 'radius')).toBeNull();
  });
});
