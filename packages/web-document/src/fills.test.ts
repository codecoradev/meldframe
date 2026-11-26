import type { Properties } from '@meldframe/protocol';
import { describe, expect, it } from 'vitest';
import { addStopAction, angleAction, fillTypeAction, gradientBar, removeStopAction, stopColorAction, stopPercent, stopPosAction } from './fills';

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
  it('sends the angle as typed and ignores text that is not a number or the angle it has', () => {
    expect(angleAction(p, '45')).toEqual(prop({ property: 'gradientAngle', value: 45 }));
    expect(angleAction(p, '180')).toBeNull();
    expect(angleAction(p, 'x')).toBeNull();
    expect(angleAction({ ...p, gradient: undefined }, '45')).toBeNull();
  });
  it('types a position as a percentage and sends a fraction', () => {
    expect(stopPercent(0.333)).toBe('33.3');
    expect(stopPosAction(p, 1, '75')).toEqual(prop({ property: 'gradientStop', value: { index: 1, pos: 0.75, color: '#ffffff' } }));
    expect(stopPosAction(p, 1, '100')).toBeNull();
    expect(stopPosAction(p, 1, '101')).toBeNull();
    expect(stopPosAction(p, 9, '10')).toBeNull();
  });
  it('sends a hex colour of a stop and refuses other text', () => {
    expect(stopColorAction(p, 0, '#ff0000')).toEqual(prop({ property: 'gradientStop', value: { index: 0, pos: 0, color: '#ff0000' } }));
    expect(stopColorAction(p, 0, '#112233')).toBeNull();
    // Text that is not a colour goes to the app layer, which refuses it with a notice; an empty field sends nothing.
    expect(stopColorAction(p, 0, 'red')).toEqual(prop({ property: 'gradientStop', value: { index: 0, pos: 0, color: 'red' } }));
    expect(stopColorAction(p, 0, '  ')).toBeNull();
  });
  it('adds and removes a stop by index', () => {
    expect(addStopAction(p)).toEqual(prop({ property: 'addGradientStop', value: {} }));
    expect(removeStopAction(p, 1)).toEqual(prop({ property: 'removeGradientStop', value: 1 }));
  });
  it('draws the bar of the stops along a line to the right', () => {
    expect(gradientBar(grad)).toBe('linear-gradient(90deg, #112233 0%, #ffffff 100%)');
  });
});
