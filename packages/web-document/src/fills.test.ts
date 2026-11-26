import type { Properties } from '@meldframe/protocol';
import { describe, expect, it } from 'vitest';
import { addStopAction, fillTypeAction, gradientBar, removeStopAction, stopPercent } from './fills';

const grad = { kind: 'linear' as const, angle: 180, stops: [{ pos: 0, color: '#112233' }, { pos: 1, color: '#ffffff' }] };
const p: Properties = { id: 'n', name: 'Box', kind: 'rect', x: 0, y: 0, w: 10, h: 10, fill: '#112233', fillKind: 'linear', gradientAllowed: true, imageAllowed: true, minStops: 2, maxStops: 8, border: { width: 0, color: '#000000', position: 'inside' }, shadows: [], maxShadows: 4, gradient: grad, radius: 0, layout: 'none', gap: 0, padding: { top: 0, right: 0, bottom: 0, left: 0 }, margin: { top: 0, right: 0, bottom: 0, left: 0 }, sw: 'fixed', sh: 'fixed', align: 'start', justify: 'start', absolute: false, inLayout: false, positionLocked: false, notes: [], sections: { radius: true, clip: false, fontSize: false, layout: false, placement: false }, text: '', fontSize: 16, opacity: 1, clip: false, bound: [], bindings: [] };
const prop = (property: object) => ({ type: 'setProperty', id: 'n', property });

describe('the type switch', () => {
  it('sends the type, and nothing for the type that is there or for the image', () => {
    expect(fillTypeAction(p, 'radial')).toEqual(prop({ property: 'fillType', value: 'radial' }));
    expect(fillTypeAction(p, 'linear')).toBeNull();
    expect(fillTypeAction(p, 'image')).toBeNull();
  });
});

describe('the gradient fields', () => {
  it('types a position as a percentage and sends a fraction', () => {
    expect(stopPercent(0.333)).toBe('33.3');
  });
  it('adds and removes a stop by index', () => {
    expect(addStopAction(p)).toEqual(prop({ property: 'addGradientStop', value: {} }));
    expect(removeStopAction(p, 1)).toEqual(prop({ property: 'removeGradientStop', value: 1 }));
  });
  it('draws the bar of the stops along a line to the right', () => {
    expect(gradientBar(grad)).toBe('linear-gradient(90deg, #112233 0%, #ffffff 100%)');
  });
});
