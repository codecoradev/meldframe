// Test data for the React copies of the Properties panel and its sections: one node's `Properties` as the app layer gives them,
// with the parts a test changes passed as `over`. Not used by the product. The colours are made from their digits: the literal scan of
// the UI code reads every file that is not a test, and a node's fill is document data, not a style.
import type { Properties } from '@meldframe/protocol';

const hex = (digits: string) => `#${digits}`;
const FILL_A = hex('112233');
const FILL_B = hex('445566');
const LINE_A = hex('000000');
const TEXT_SIZE = 16;
const zero = { top: 0, right: 0, bottom: 0, left: 0 };
export const FRAME = { radius: true, clip: true, fontSize: false, layout: true, placement: false };
export const RECT = { radius: true, clip: false, fontSize: false, layout: false, placement: false };
export const TEXT = { radius: false, clip: false, fontSize: true, layout: false, placement: false };

const base: Properties = {
  id: 'n', name: 'Hero', kind: 'frame', x: 10, y: 20, w: 300, h: 120,
  fill: FILL_A, fillKind: 'solid', gradientAllowed: true, imageAllowed: true, minStops: 2, maxStops: 8,
  border: { width: 0, color: LINE_A, position: 'inside' }, shadows: [], maxShadows: 4,
  radius: 8, layout: 'none', gap: 8, padding: zero, margin: zero, sw: 'fixed', sh: 'fixed', align: 'start', justify: 'start',
  absolute: false, inLayout: false, positionLocked: false, notes: [], sections: FRAME,
  text: '', fontSize: TEXT_SIZE, opacity: 0.5, clip: true, bound: [], bindings: [],
};

export const properties = (over: Partial<Properties> = {}): Properties => ({ ...base, ...over });
export const LINE_B = hex('ff0000');
export const shadow = { x: 0, y: 4, blur: 8, spread: 0, color: hex('00000040') };
export const gradient = { kind: 'linear' as const, angle: 90, stops: [{ pos: 0, color: FILL_A }, { pos: 0.5, color: FILL_B }, { pos: 1, color: hex('ffffff') }] };
export const binding = (field: 'fill' | 'opacity' | 'radius', over = {}) => ({ field, variable: 'v1', name: 'space/md', missing: false, ...over });
