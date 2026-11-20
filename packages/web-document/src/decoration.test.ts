import type { Properties } from '@meldframe/protocol';
import { describe, expect, it } from 'vitest';
import { addShadowAction, borderColorAction, borderPositionAction, borderWidthAction, removeShadowAction, shadowAction } from './decoration';

const shadow = { x: 0, y: 4, blur: 8, spread: 0, color: '#00000040' };
const p: Properties = { id: 'n', name: 'Box', kind: 'rect', x: 0, y: 0, w: 10, h: 10, fill: '#112233', fillKind: 'solid', gradientAllowed: true, minStops: 2, maxStops: 8, border: { width: 1, color: '#000000', position: 'inside' }, shadows: [shadow], maxShadows: 4, radius: 0, text: '', fontSize: 16, opacity: 1, clip: false, bound: [], bindings: [] };
const prop = (property: object) => ({ type: 'setProperty', id: 'n', property });

describe('the border', () => {
  it('sends a changed width, and nothing for text, a negative number or the width it has', () => {
    expect(borderWidthAction(p, '3')).toEqual(prop({ property: 'borderWidth', value: 3 }));
    expect(borderWidthAction(p, '1')).toBeNull();
    expect(borderWidthAction(p, '-1')).toBeNull();
    expect(borderWidthAction(p, 'x')).toBeNull();
    expect(borderWidthAction(p, '')).toBeNull();
  });
  it('sends a colour as typed, for the app layer to take or refuse, and nothing when it is empty or unchanged', () => {
    expect(borderColorAction(p, '#ff0000')).toEqual(prop({ property: 'borderColor', value: '#ff0000' }));
    expect(borderColorAction(p, '#f00')).not.toBeNull();
    expect(borderColorAction(p, '#FF000080')).not.toBeNull();
    expect(borderColorAction(p, '#000000')).toBeNull();
    // Text that is not a colour goes to the app layer, which refuses it with a notice; an empty field sends nothing.
    expect(borderColorAction(p, 'red')).toEqual(prop({ property: 'borderColor', value: 'red' }));
    expect(borderColorAction(p, ' ')).toBeNull();
  });
  it('sends a position other than the one it has', () => {
    expect(borderPositionAction(p, 'outside')).toEqual(prop({ property: 'borderPosition', value: 'outside' }));
    expect(borderPositionAction(p, 'inside')).toBeNull();
  });
});

describe('the shadows', () => {
  it('sends the whole shadow with the one field changed', () => {
    expect(shadowAction(p, 0, 'blur', '20')).toEqual(prop({ property: 'setShadow', value: { index: 0, shadow: { ...shadow, blur: 20 } } }));
    expect(shadowAction(p, 0, 'x', '-3')).toEqual(prop({ property: 'setShadow', value: { index: 0, shadow: { ...shadow, x: -3 } } }));
    expect(shadowAction(p, 0, 'color', '#112233')).toEqual(prop({ property: 'setShadow', value: { index: 0, shadow: { ...shadow, color: '#112233' } } }));
  });
  it('sends nothing for an unchanged value, a negative blur, a shadow that is not there or text that is not valid', () => {
    expect(shadowAction(p, 0, 'y', '4')).toBeNull();
    expect(shadowAction(p, 0, 'blur', '-1')).toBeNull();
    expect(shadowAction(p, 1, 'x', '1')).toBeNull();
    expect(shadowAction(p, 0, 'spread', 'a')).toBeNull();
    expect(shadowAction(p, 0, 'color', '')).toBeNull();
  });
  it('adds and removes by index', () => {
    expect(addShadowAction(p)).toEqual(prop({ property: 'addShadow', value: {} }));
    expect(removeShadowAction(p, 0)).toEqual(prop({ property: 'removeShadow', value: 0 }));
  });
});
