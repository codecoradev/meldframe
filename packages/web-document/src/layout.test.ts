import type { LayoutNote, Properties } from '@meldframe/protocol';
import { describe, expect, it } from 'vitest';
import { absoluteAction, alignAction, justifyAction, layoutAction, noteText, sizingAction } from './layout';

const sides = { top: 1, right: 2, bottom: 3, left: 4 };
const p: Properties = { id: 'n', name: 'Box', kind: 'frame', x: 0, y: 0, w: 10, h: 10, fill: '#112233', fillKind: 'solid', gradientAllowed: true, imageAllowed: true, minStops: 2, maxStops: 8, border: { width: 0, color: '#000000', position: 'inside' }, shadows: [], maxShadows: 4, radius: 0, layout: 'row', gap: 8, padding: sides, margin: { top: 0, right: 0, bottom: 0, left: 0 }, sw: 'fixed', sh: 'hug', align: 'start', justify: 'start', absolute: false, inLayout: true, positionLocked: false, notes: [], sections: { radius: true, clip: false, fontSize: false, layout: false, placement: false }, text: '', fontSize: 16, opacity: 1, clip: true, bound: [], bindings: [] };
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
