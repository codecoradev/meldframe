import type { Properties } from '@meldframe/protocol';
import { describe, expect, it } from 'vitest';
import { boxAction, bindingOf, clipAction, fieldsOf, percent, propertyAction, unbindAction } from './properties';

const p: Properties = { id: 'n', name: 'Box', kind: 'rect', x: 10, y: 20, w: 100, h: 50, fill: '#112233', fillKind: 'solid', gradientAllowed: true, minStops: 2, maxStops: 8, border: { width: 0, color: '#000000', position: 'inside' }, shadows: [], maxShadows: 4, radius: 4, layout: 'none', gap: 0, padding: { top: 0, right: 0, bottom: 0, left: 0 }, margin: { top: 0, right: 0, bottom: 0, left: 0 }, sw: 'fixed', sh: 'fixed', align: 'start', justify: 'start', absolute: false, inLayout: false, notes: [], text: '', fontSize: 16, opacity: 1, clip: true, bound: [], bindings: [] };

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
