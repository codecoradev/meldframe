import type { Node } from '@meldframe/protocol';
import type { CreateIntent, MoveIntent, ResizeIntent } from '@meldframe/web-canvas/interaction';
import type { Scene } from '@meldframe/web-canvas/scene';
import { describe, expect, it } from 'vitest';
import { containerFor, createActionFor, keyAction, moveActionsFor, previewScene, resizeActionFor, resizedBox, toolMode, worldOrigin } from './edit';

const node = (id: string, over: Partial<Node> = {}): Node =>
  ({ id, name: id, type: 'rect', parentId: null, x: 0, y: 0, w: 100, h: 50, ...over }) as Node;
const gesture = { final: true, cancelled: false, mods: { shift: false, alt: false, ctrl: false, meta: false } };

describe('resizedBox', () => {
  const b = { x: 10, y: 20, w: 100, h: 50 };
  it('moves the edge the handle holds and leaves the opposite one', () => {
    expect(resizedBox(b, 'se', 10, 5)).toEqual({ x: 10, y: 20, w: 110, h: 55 });
    expect(resizedBox(b, 'nw', -10, -5)).toEqual({ x: 0, y: 15, w: 110, h: 55 });
    expect(resizedBox(b, 'e', 7, 99)).toEqual({ x: 10, y: 20, w: 107, h: 50 });
    expect(resizedBox(b, 'n', 99, -4)).toEqual({ x: 10, y: 16, w: 100, h: 54 });
    expect(resizedBox(b, 'sw', -3, 3)).toEqual({ x: 7, y: 20, w: 103, h: 53 });
  });
  it('never goes under the minimum size and keeps the opposite edge where it is', () => {
    expect(resizedBox(b, 'e', -500, 0)).toEqual({ x: 10, y: 20, w: 1, h: 50 });
    expect(resizedBox(b, 'w', 500, 0)).toEqual({ x: 109, y: 20, w: 1, h: 50 });
    expect(resizedBox(b, 'n', 0, 500)).toEqual({ x: 10, y: 69, w: 100, h: 1 });
    expect(resizedBox(b, 's', 0, -500)).toEqual({ x: 10, y: 20, w: 100, h: 1 });
  });
});

describe('parents and spaces', () => {
  const nodes = [node('f', { type: 'frame', x: 10, y: 20 }), node('g', { type: 'frame', parentId: 'f', x: 5, y: 6 }), node('r', { parentId: 'g' }), node('o')];
  it('adds up the origins along the parents', () => {
    expect(worldOrigin(nodes, null)).toEqual({ x: 0, y: 0 });
    expect(worldOrigin(nodes, 'g')).toEqual({ x: 15, y: 26 });
    expect(worldOrigin(nodes, 'ghost')).toEqual({ x: 0, y: 0 });
  });
  it('a cycle in a bad file does not loop', () => {
    const bad = [node('a', { parentId: 'b' }), node('b', { parentId: 'a' })];
    expect(worldOrigin(bad, 'a')).toEqual({ x: 0, y: 0 });
  });
  it('a node goes into the nearest frame at or above the node under the press, else on the canvas', () => {
    expect(containerFor('g', nodes)).toBe('g');
    expect(containerFor('r', nodes)).toBe('g'); // a rectangle holds no children
    expect(containerFor('o', nodes)).toBeNull();
    expect(containerFor(null, nodes)).toBeNull();
  });
});

describe('createActionFor', () => {
  const nodes = [node('f', { type: 'frame', x: 10, y: 20 })];
  const rubber = (over: Partial<CreateIntent>): CreateIntent => ({ ...gesture, tool: 'rect', at: null, x: 0, y: 0, w: 0, h: 0, ...over });
  it('a click makes a node of the default size at the point, in the space of the frame under it', () => {
    expect(createActionFor(rubber({ tool: 'frame', at: 'f', x: 50, y: 60 }), nodes)).toEqual({ type: 'createNode', kind: 'frame', x: 40, y: 40, parent: 'f' });
    expect(createActionFor(rubber({ tool: 'rect', x: 5, y: 6 }), nodes)).toEqual({ type: 'createNode', kind: 'rectangle', x: 5, y: 6, parent: null });
  });
  it('a dragged box carries its size in the same action', () => {
    expect(createActionFor(rubber({ tool: 'text', at: 'f', x: 30, y: 40, w: 200, h: 80 }), nodes)).toEqual({
      type: 'createNode', kind: 'text', x: 20, y: 20, parent: 'f', w: 200, h: 80,
    });
  });
  it('a small drag that the canvas started is a drawn box, not a click (a click is reported with size 0)', () => {
    expect(createActionFor(rubber({ x: 5, y: 6, w: 2, h: 3 }), nodes)).toEqual({ type: 'createNode', kind: 'rectangle', x: 5, y: 6, parent: null, w: 2, h: 3 });
  });
  it('a box dragged flat in one direction still gets a size that can be grabbed', () => {
    expect(createActionFor(rubber({ x: 5, y: 6, w: 100, h: 0 }), nodes)).toEqual({ type: 'createNode', kind: 'rectangle', x: 5, y: 6, parent: null, w: 100, h: 1 });
  });
  it('a cancelled gesture and one that is not final make nothing', () => {
    expect(createActionFor(rubber({ cancelled: true }), nodes)).toBeNull();
    expect(createActionFor(rubber({ final: false }), nodes)).toBeNull();
  });
});

describe('moveActionsFor', () => {
  const nodes = [node('a', { x: 1, y: 2 }), node('b', { x: 10, y: 20 })];
  const mv = (over: Partial<MoveIntent>): MoveIntent => ({ ...gesture, ids: ['a'], dx: 5, dy: -3, ...over });
  it('moves the whole selection as one step', () => {
    expect(moveActionsFor(mv({ ids: ['b', 'a'] }), ['a', 'b'], nodes)).toEqual([{ type: 'moveBy', dx: 5, dy: -3 }]);
  });
  it('moves nodes that are not the selection one by one to their new place', () => {
    expect(moveActionsFor(mv({ ids: ['a'] }), ['a', 'b'], nodes)).toEqual([{ type: 'move', id: 'a', x: 6, y: -1 }]);
  });
  it('moves only the top node when a node and one of its descendants are both selected (the descendant follows its parent)', () => {
    const tree = [node('f', { type: 'frame', x: 10, y: 20 }), node('c', { parentId: 'f', x: 1, y: 1 }), node('g', { type: 'frame', parentId: 'f' }), node('d', { parentId: 'g' })];
    expect(moveActionsFor(mv({ ids: ['f', 'c'] }), ['f', 'c'], tree)).toEqual([{ type: 'move', id: 'f', x: 15, y: 17 }]);
    expect(moveActionsFor(mv({ ids: ['f', 'd', 'c'] }), ['f', 'd', 'c'], tree)).toEqual([{ type: 'move', id: 'f', x: 15, y: 17 }]);
    // Two nodes that are not related are still one step.
    expect(moveActionsFor(mv({ ids: ['c', 'g'] }), ['c', 'g'], tree)).toEqual([{ type: 'moveBy', dx: 5, dy: -3 }]);
  });
  it('a gesture that did not move, was cancelled, or is not final does nothing', () => {
    expect(moveActionsFor(mv({ dx: 0, dy: 0 }), ['a'], nodes)).toEqual([]);
    expect(moveActionsFor(mv({ cancelled: true }), ['a'], nodes)).toEqual([]);
    expect(moveActionsFor(mv({ final: false }), ['a'], nodes)).toEqual([]);
  });
});

describe('resizeActionFor', () => {
  const nodes = [node('a', { x: 10, y: 20, w: 100, h: 50 })];
  const rz = (over: Partial<ResizeIntent>): ResizeIntent => ({ ...gesture, ids: ['a'], handle: 'se', dx: 10, dy: 5, ...over });
  it('sends the size alone when the origin stays', () => {
    expect(resizeActionFor(rz({}), nodes)).toEqual({ type: 'resize', id: 'a', w: 110, h: 55 });
  });
  it('sends the origin with the size when the left or top edge moves: one action', () => {
    expect(resizeActionFor(rz({ handle: 'nw', dx: -10, dy: -5 }), nodes)).toEqual({ type: 'resize', id: 'a', w: 110, h: 55, x: 0, y: 15 });
  });
  it('resizes one node only, and nothing for a gesture that changed nothing, was cancelled or is not final', () => {
    expect(resizeActionFor(rz({ ids: ['a', 'b'] }), nodes)).toBeNull();
    expect(resizeActionFor(rz({ dx: 0, dy: 0 }), nodes)).toBeNull();
    expect(resizeActionFor(rz({ cancelled: true }), nodes)).toBeNull();
    expect(resizeActionFor(rz({ final: false }), nodes)).toBeNull();
    expect(resizeActionFor(rz({ ids: ['ghost'] }), nodes)).toBeNull();
  });
});

describe('previewScene', () => {
  const scene: Scene = { space: 'parent', nodes: [
    { id: 'a', parent: null, kind: 'rect', x: 1, y: 2, w: 10, h: 10, fill: 1 },
    { id: 'b', parent: null, kind: 'rect', x: 5, y: 5, w: 10, h: 10, fill: 2 },
  ] };
  it('shows a move on the nodes that move and no other, and changes nothing without a preview', () => {
    expect(previewScene(scene, null)).toBe(scene);
    const s = previewScene(scene, { kind: 'move', ids: ['a'], dx: 3, dy: 4 });
    expect(s.nodes.map((n) => [n.id, n.x, n.y])).toEqual([['a', 4, 6], ['b', 5, 5]]);
  });
  it('shows a resized box', () => {
    const s = previewScene(scene, { kind: 'box', id: 'b', box: { x: 0, y: 1, w: 2, h: 3 } });
    expect(s.nodes[1]).toMatchObject({ x: 0, y: 1, w: 2, h: 3 });
  });
  it('moves only the top node of a moved node and its descendants: the descendants follow their parent', () => {
    const tree: Scene = { space: 'parent', nodes: [
      { id: 'f', parent: null, kind: 'frame', x: 10, y: 10, w: 50, h: 50, fill: 1 },
      { id: 'c', parent: 'f', kind: 'rect', x: 1, y: 1, w: 5, h: 5, fill: 2 },
    ] };
    const s = previewScene(tree, { kind: 'move', ids: ['f', 'c'], dx: 3, dy: 4 });
    expect(s.nodes.map((n) => [n.id, n.x, n.y])).toEqual([['f', 13, 14], ['c', 1, 1]]);
  });
});

describe('keyAction and toolMode', () => {
  const k = (key: string, over: object = {}) => ({ key, metaKey: false, ctrlKey: false, shiftKey: false, altKey: false, ...over });
  it('picks a tool with a plain letter', () => {
    expect(keyAction(k('v'), [])).toEqual({ type: 'setTool', tool: 'select' });
    expect(keyAction(k('F'), [])).toEqual({ type: 'setTool', tool: 'frame' });
    expect(keyAction(k('r'), [])).toEqual({ type: 'setTool', tool: 'rectangle' });
    expect(keyAction(k('t'), [])).toEqual({ type: 'setTool', tool: 'text' });
    expect(keyAction(k('r', { metaKey: true }), [])).toBeNull(); // the browser's reload
  });
  it('deletes the selection, and does nothing when there is none', () => {
    expect(keyAction(k('Delete'), ['a', 'b'])).toEqual({ type: 'delete', ids: ['a', 'b'] });
    expect(keyAction(k('Backspace'), ['a'])).toEqual({ type: 'delete', ids: ['a'] });
    expect(keyAction(k('Delete'), [])).toBeNull();
  });
  it('undoes and redoes with the platform key', () => {
    expect(keyAction(k('z', { metaKey: true }), [])).toEqual({ type: 'undo' });
    expect(keyAction(k('z', { ctrlKey: true }), [])).toEqual({ type: 'undo' });
    expect(keyAction(k('z', { metaKey: true, shiftKey: true }), [])).toEqual({ type: 'redo' });
    expect(keyAction(k('Z', { ctrlKey: true, shiftKey: true }), [])).toEqual({ type: 'redo' });
    expect(keyAction(k('y', { ctrlKey: true }), [])).toEqual({ type: 'redo' });
    expect(keyAction(k('z'), [])).toBeNull();
  });
  it('maps the tool of the app to the mode of the canvas', () => {
    expect([toolMode('select'), toolMode('frame'), toolMode('rectangle'), toolMode('text')]).toEqual(['move', 'frame', 'rect', 'text']);
  });
});
