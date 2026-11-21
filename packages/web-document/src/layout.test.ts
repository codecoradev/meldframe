import type { LayoutNote, Properties } from '@meldframe/protocol';
import { describe, expect, it } from 'vitest';
import { absoluteAction, alignAction, gapAction, justifyAction, layoutAction, marginSideAction, noteText, paddingAllAction, showPlacement, paddingSideAction, sizingAction } from './layout';

const sides = { top: 1, right: 2, bottom: 3, left: 4 };
const p: Properties = { id: 'n', name: 'Box', kind: 'frame', x: 0, y: 0, w: 10, h: 10, fill: '#112233', fillKind: 'solid', gradientAllowed: true, minStops: 2, maxStops: 8, border: { width: 0, color: '#000000', position: 'inside' }, shadows: [], maxShadows: 4, radius: 0, layout: 'row', gap: 8, padding: sides, margin: { top: 0, right: 0, bottom: 0, left: 0 }, sw: 'fixed', sh: 'hug', align: 'start', justify: 'start', absolute: false, inLayout: true, positionLocked: false, notes: [], text: '', fontSize: 16, opacity: 1, clip: true, bound: [], bindings: [] };
const prop = (property: object) => ({ type: 'setProperty', id: 'n', property });

describe('the choices', () => {
  it('send a value that differs and nothing for the one the node has', () => {
    expect(layoutAction(p, 'column')).toEqual(prop({ property: 'layout', value: 'column' }));
    expect(layoutAction(p, 'row')).toBeNull();
    expect(alignAction(p, 'center')).toEqual(prop({ property: 'align', value: 'center' }));
    expect(alignAction(p, 'start')).toBeNull();
    expect(justifyAction(p, 'between')).toEqual(prop({ property: 'justify', value: 'between' }));
    expect(absoluteAction(p, true)).toEqual(prop({ property: 'absolute', value: true }));
    expect(absoluteAction(p, false)).toBeNull();
  });
  it('size one axis at a time', () => {
    expect(sizingAction(p, 'width', 'fill')).toEqual(prop({ property: 'sizing', value: { axis: 'width', mode: 'fill' } }));
    expect(sizingAction(p, 'height', 'hug')).toBeNull();
    expect(sizingAction(p, 'width', 'hug')).not.toBeNull();
  });
});

describe('the lengths', () => {
  it('take a number of 0 or more and refuse text, a negative number, empty text and the value they have', () => {
    expect(gapAction(p, '12')).toEqual(prop({ property: 'gap', value: 12 }));
    expect(gapAction(p, '0')).toEqual(prop({ property: 'gap', value: 0 }));
    for (const bad of ['-1', 'x', '', ' ', '8']) expect(gapAction(p, bad), bad).toBeNull();
  });
  it('pad and margin one side', () => {
    expect(paddingSideAction(p, 'left', '9')).toEqual(prop({ property: 'padding', value: { side: 'left', value: 9 } }));
    expect(paddingSideAction(p, 'left', '4')).toBeNull();
    expect(marginSideAction(p, 'top', '5')).toEqual(prop({ property: 'margin', value: { side: 'top', value: 5 } }));
    expect(marginSideAction(p, 'top', '0')).toBeNull();
  });
  it('pad all sides from the uniform field, also when the sides differ', () => {
    expect(paddingAllAction(p, '6')).toEqual(prop({ property: 'paddingAll', value: 6 }));
    expect(paddingAllAction({ ...p, padding: { top: 6, right: 6, bottom: 6, left: 6 }, paddingUniform: 6 }, '6')).toBeNull();
    expect(paddingAllAction(p, '-2')).toBeNull();
  });
});

describe('showPlacement', () => {
  it('is on in a layout, and out of one only while the node has a margin or is absolute', () => {
    expect(showPlacement({ ...p, inLayout: true })).toBe(true);
    expect(showPlacement({ ...p, inLayout: false })).toBe(false);
    expect(showPlacement({ ...p, inLayout: false, absolute: true })).toBe(true);
    expect(showPlacement({ ...p, inLayout: false, margin: { top: 0, right: 0, bottom: 2, left: 0 } })).toBe(true);
  });
});

describe('the notes', () => {
  it('say in words what has no effect, naming the axis', () => {
    const notes: LayoutNote[] = [{ note: 'fillUnderHug', axis: 'width' }, { note: 'hugIgnored', axis: 'height' }, { note: 'marginIgnored' }, { note: 'absoluteIgnored' }, { note: 'absoluteFillIgnored', axis: 'height' }];
    const text = notes.map(noteText);
    expect(text[0]).toMatch(/Fill width/);
    expect(text[1]).toMatch(/Hug height/);
    expect(text[2]).toMatch(/margin/i);
    expect(text[2]).toMatch(/absolute/);
    expect(text[3]).toMatch(/Absolute/);
    expect(text[4]).toMatch(/Fill height has no effect on an absolute node/);
    expect(new Set(text).size).toBe(5);
  });
});
