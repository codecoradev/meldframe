import type { Properties } from '@meldframe/protocol';
import { describe, expect, it } from 'vitest';
import { boxAction, bindingOf, clipAction, fieldsOf, FONT_WEIGHTS, fontWeightAction, percent, propertyAction, TEXT_ALIGNS, textAlignAction, unbindAction } from './properties';

const p: Properties = { id: 'n', name: 'Box', kind: 'rect', x: 10, y: 20, w: 100, h: 50, fill: '#112233', fillKind: 'solid', gradientAllowed: true, imageAllowed: true, minStops: 2, maxStops: 8, border: { width: 0, color: '#000000', position: 'inside' }, shadows: [], maxShadows: 4, radius: 4, layout: 'none', gap: 0, padding: { top: 0, right: 0, bottom: 0, left: 0 }, margin: { top: 0, right: 0, bottom: 0, left: 0 }, sw: 'fixed', sh: 'fixed', align: 'start', justify: 'start', absolute: false, inLayout: false, positionLocked: false, notes: [], text: '', fontSize: 16, opacity: 1, clip: true, bound: [], bindings: [] };

describe('propertyAction', () => {
  it('sets a changed name, trimmed', () => {
    expect(propertyAction(p, 'name', '  Card ')).toEqual({ type: 'setProperty', id: 'n', property: { property: 'name', value: 'Card' } });
  });
  it('does nothing for an unchanged or empty name', () => {
    expect(propertyAction(p, 'name', 'Box')).toBeNull();
    expect(propertyAction(p, 'name', '   ')).toBeNull();
  });
  it('sets a hex fill and refuses anything else', () => {
    expect(propertyAction(p, 'fill', '#abc')).toEqual({ type: 'setProperty', id: 'n', property: { property: 'fill', value: '#abc' } });
    expect(propertyAction(p, 'fill', '#112233')).toBeNull();
    expect(propertyAction(p, 'fill', 'none')).toEqual({ type: 'setProperty', id: 'n', property: { property: 'fill', value: 'none' } });
    expect(propertyAction({ ...p, fill: 'none' }, 'fill', 'none')).toBeNull();
    expect(propertyAction(p, 'fill', 'red')).toBeNull();
    expect(propertyAction(p, 'fill', '#12')).toBeNull();
  });
  it('sets the radius and the font size as numbers, never negative or zero where that makes no sense', () => {
    expect(propertyAction(p, 'radius', '8')).toEqual({ type: 'setProperty', id: 'n', property: { property: 'radius', value: 8 } });
    expect(propertyAction(p, 'radius', '0')).toEqual({ type: 'setProperty', id: 'n', property: { property: 'radius', value: 0 } });
    expect(propertyAction(p, 'radius', '-1')).toBeNull();
    expect(propertyAction(p, 'fontSize', '0')).toBeNull();
    expect(propertyAction(p, 'fontSize', '24.5')).toEqual({ type: 'setProperty', id: 'n', property: { property: 'fontSize', value: 24.5 } });
    expect(propertyAction(p, 'fontSize', 'abc')).toBeNull();
    expect(propertyAction(p, 'radius', '4')).toBeNull();
  });
});

describe('boxAction', () => {
  it('moves for x or y and keeps the other coordinate', () => {
    expect(boxAction(p, 'x', '30')).toEqual({ type: 'move', id: 'n', x: 30, y: 20 });
    expect(boxAction(p, 'y', '-5')).toEqual({ type: 'move', id: 'n', x: 10, y: -5 });
  });
  it('resizes for w or h and keeps the other side and the origin', () => {
    expect(boxAction(p, 'w', '120')).toEqual({ type: 'resize', id: 'n', w: 120, h: 50 });
    expect(boxAction(p, 'h', '10')).toEqual({ type: 'resize', id: 'n', w: 100, h: 10 });
  });
  it('does nothing for x or y while the position comes from the layout (a move would write a stored position that nothing reads), and still resizes', () => {
    const locked = { ...p, positionLocked: true };
    expect(boxAction(locked, 'x', '30')).toBeNull();
    expect(boxAction(locked, 'y', '-5')).toBeNull();
    expect(boxAction(locked, 'w', '120')).toEqual({ type: 'resize', id: 'n', w: 120, h: 50 });
    expect(boxAction(locked, 'h', '10')).toEqual({ type: 'resize', id: 'n', w: 100, h: 10 });
  });
  it('does nothing for an unchanged, empty, non-numeric or too small value', () => {
    expect(boxAction(p, 'x', '10')).toBeNull();
    expect(boxAction(p, 'x', '')).toBeNull();
    expect(boxAction(p, 'w', 'wide')).toBeNull();
    expect(boxAction(p, 'w', '0')).toBeNull();
    expect(boxAction(p, 'h', '-4')).toBeNull();
  });
});

describe('opacity', () => {
  it('is typed as a percentage and sent as 0 to 1', () => {
    expect(propertyAction(p, 'opacity', '50')).toEqual({ type: 'setProperty', id: 'n', property: { property: 'opacity', value: 0.5 } });
    expect(propertyAction(p, 'opacity', '0')).toEqual({ type: 'setProperty', id: 'n', property: { property: 'opacity', value: 0 } });
    expect(propertyAction(p, 'opacity', '33.3')).toEqual({ type: 'setProperty', id: 'n', property: { property: 'opacity', value: 0.333 } });
  });
  it('does nothing for an unchanged value, text, or a value outside 0 to 100', () => {
    expect(propertyAction(p, 'opacity', '100')).toBeNull();
    expect(propertyAction({ ...p, opacity: 0.5 }, 'opacity', '50')).toBeNull();
    expect(propertyAction(p, 'opacity', 'half')).toBeNull();
    expect(propertyAction(p, 'opacity', '101')).toBeNull();
    expect(propertyAction(p, 'opacity', '-1')).toBeNull();
  });
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

describe('fieldsOf', () => {
  it('offers the fields a kind of node has', () => {
    expect(fieldsOf('frame')).toEqual({ radius: true, clip: true, fontSize: false, layout: true });
    expect(fieldsOf('rect')).toEqual({ radius: true, clip: false, fontSize: false, layout: false });
    expect(fieldsOf('ellipse')).toEqual({ radius: false, clip: false, fontSize: false, layout: false });
    expect(fieldsOf('text')).toEqual({ radius: false, clip: false, fontSize: true, layout: false });
  });
});
