import type { Properties } from '@meldframe/protocol';
import { describe, expect, it } from 'vitest';
import { addShadowAction, borderPositionAction, removeBorderAction, removeShadowAction, shadowAction } from './decoration';

const shadow = { x: 0, y: 4, blur: 8, spread: 0, color: '#00000040' };
const p: Properties = { id: 'n', name: 'Box', kind: 'rect', x: 0, y: 0, w: 10, h: 10, fill: '#112233', fillKind: 'solid', gradientAllowed: true, imageAllowed: true, minStops: 2, maxStops: 8, border: { width: 1, color: '#000000', position: 'inside' }, shadows: [shadow], maxShadows: 4, radius: 0, layout: 'none', gap: 0, padding: { top: 0, right: 0, bottom: 0, left: 0 }, margin: { top: 0, right: 0, bottom: 0, left: 0 }, sw: 'fixed', sh: 'fixed', align: 'start', justify: 'start', absolute: false, inLayout: false, positionLocked: false, notes: [], sections: { radius: true, clip: false, fontSize: false, layout: false, placement: false }, text: '', fontSize: 16, opacity: 1, clip: false, bound: [], bindings: [] };
const prop = (property: object) => ({ type: 'setProperty', id: 'n', property });

describe('the border', () => {
  it('sends a position other than the one it has', () => {
    expect(borderPositionAction(p, 'outside')).toEqual(prop({ property: 'borderPosition', value: 'outside' }));
    expect(borderPositionAction(p, 'inside')).toBeNull();
  });
  it('removes the border with the one text the width field sends: 0, and no colour or position', () => {
    expect(removeBorderAction(p)).toEqual({ type: 'typed', entry: { type: 'property', id: 'n', field: { type: 'borderWidth' }, text: '0' } });
    // A width that is not above 0 has no border to remove: nothing is sent.
    expect(removeBorderAction({ ...p, border: { ...p.border, width: 0 } })).toBeNull();
  });
});

describe('the shadows', () => {
  it('sends the part of the shadow that was typed, as typed: the ranges are the app layer\'s', () => {
    expect(shadowAction(p, 0, 'blur', '-1')).toEqual({ type: 'typed', entry: { type: 'property', id: 'n', field: { type: 'shadow', index: 0, part: { type: 'blur' } }, text: '-1' } });
    expect(shadowAction(p, 2, 'color', ' #fff ')).toMatchObject({ entry: { field: { type: 'shadow', index: 2, part: { type: 'color' } }, text: ' #fff ' } });
  });
  it('adds and removes by index', () => {
    expect(addShadowAction(p)).toEqual(prop({ property: 'addShadow', value: {} }));
    expect(removeShadowAction(p, 0)).toEqual(prop({ property: 'removeShadow', value: 0 }));
  });
});
