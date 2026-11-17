import type { Properties } from '@meldframe/protocol';
import { describe, expect, it } from 'vitest';
import { boxAction, propertyAction } from './properties';

const p: Properties = { id: 'n', name: 'Box', kind: 'rect', x: 10, y: 20, w: 100, h: 50, fill: '#112233', radius: 4, text: '', fontSize: 16 };

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
