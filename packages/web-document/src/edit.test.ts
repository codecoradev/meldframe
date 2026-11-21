import type { Node } from '@meldframe/protocol';
import type { CreateIntent, MoveIntent, ResizeIntent } from '@meldframe/web-canvas/interaction';
import type { Scene } from '@meldframe/web-canvas/scene';
import { describe, expect, it } from 'vitest';
import { OWNS_KEYS_SELECTOR, containerFor, createActionFor, deleteAction, drawnBox, keyAction, movePreviewFor, moveActionsFor, ownsKeys, previewScene, resizeActionFor, resizePreviewFor, resizedBox, toolMode, worldOrigin } from './edit';

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
  const at = { x: 40, y: 50 };
  const mv = (over: Partial<MoveIntent>): MoveIntent => ({ ...gesture, ids: ['a'], dx: 5, dy: -3, at, ...over });
  it('moves the whole selection as one step', () => {
    expect(moveActionsFor(mv({ ids: ['b', 'a'] }), ['a', 'b'], nodes)).toEqual([{ type: 'moveBy', dx: 5, dy: -3 }]);
  });
  it('drops one dragged node where the pointer is: the app layer decides between a move, a reorder and a reparent', () => {
    expect(moveActionsFor(mv({ ids: ['a'] }), ['a'], nodes)).toEqual([{ type: 'dropNode', id: 'a', x: 40, y: 50, dx: 5, dy: -3 }]);
    expect(moveActionsFor(mv({ ids: ['a'] }), ['a', 'b'], nodes)).toEqual([{ type: 'dropNode', id: 'a', x: 40, y: 50, dx: 5, dy: -3 }]);
  });
  it('drops only the top node when a node and one of its descendants are both selected (the descendant follows its parent)', () => {
    const tree = [node('f', { type: 'frame', x: 10, y: 20 }), node('c', { parentId: 'f', x: 1, y: 1 }), node('g', { type: 'frame', parentId: 'f' }), node('d', { parentId: 'g' })];
    const drop = [{ type: 'dropNode', id: 'f', x: 40, y: 50, dx: 5, dy: -3 }];
    expect(moveActionsFor(mv({ ids: ['f', 'c'] }), ['f', 'c'], tree)).toEqual(drop);
    expect(moveActionsFor(mv({ ids: ['f', 'd', 'c'] }), ['f', 'd', 'c'], tree)).toEqual(drop);
    // Two nodes that are not related are still one step.
    expect(moveActionsFor(mv({ ids: ['c', 'g'] }), ['c', 'g'], tree)).toEqual([{ type: 'moveBy', dx: 5, dy: -3 }]);
  });
  it('moves several nodes that are not the selection one by one to their new place', () => {
    expect(moveActionsFor(mv({ ids: ['a', 'b'] }), ['a'], nodes)).toEqual([{ type: 'move', id: 'a', x: 6, y: -1 }, { type: 'move', id: 'b', x: 15, y: 17 }]);
  });
  it('a gesture that did not move, was cancelled, or is not final does nothing', () => {
    expect(moveActionsFor(mv({ dx: 0, dy: 0 }), ['a'], nodes)).toEqual([]);
    expect(moveActionsFor(mv({ cancelled: true }), ['a'], nodes)).toEqual([]);
    expect(moveActionsFor(mv({ final: false }), ['a'], nodes)).toEqual([]);
    expect(moveActionsFor(mv({ ids: ['ghost'] }), ['ghost'], nodes)).toEqual([]);
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

// A laid-out file: the frame `f` is stored at (10, 20) but drawn at (110, 120) and its child `c` is stored at (0, 0)
// with size 1 by 1 (hug, fill) but drawn at (5, 6), 40 by 30 in the frame.
const drawnNodes = [node('f', { type: 'frame', x: 10, y: 20, w: 200, h: 100 }), node('c', { parentId: 'f', x: 0, y: 0, w: 1, h: 1 })];
const drawn: Scene = { space: 'parent', nodes: [
  { id: 'f', parent: null, kind: 'frame', x: 110, y: 120, w: 200, h: 100, fill: 1 },
  { id: 'c', parent: 'f', kind: 'rect', x: 5, y: 6, w: 40, h: 30, fill: 2 },
] };
/** What the canvas draws with the host flag off: the stored boxes. */
const storedScene = (nodes: readonly Node[]): Scene => ({ space: 'parent', nodes: nodes.map((n) => ({ id: n.id, parent: n.parentId, kind: n.type as 'rect', x: n.x, y: n.y, w: n.w, h: n.h, fill: 0 })) });

describe('drawn boxes', () => {
  it('reads the box a node is drawn at from the scene, in the space of its parent', () => {
    expect(drawnBox(drawn, 'c')).toEqual({ x: 5, y: 6, w: 40, h: 30 });
    expect(drawnBox(drawn, 'ghost')).toBeNull();
    expect(drawnBox(undefined, 'c')).toBeNull();
    expect(drawnBox({ ...drawn, space: 'world' }, 'c')).toBeNull(); // not the form the engine hands over: the stored box is used
  });
  it('the origin of a parent is the drawn one when the scene has it, else the stored one', () => {
    expect(worldOrigin(drawnNodes, 'f')).toEqual({ x: 10, y: 20 });
    expect(worldOrigin(drawnNodes, 'f', drawn)).toEqual({ x: 110, y: 120 });
    expect(worldOrigin(drawnNodes, 'c', drawn)).toEqual({ x: 115, y: 126 });
    expect(worldOrigin(drawnNodes, 'c', { ...drawn, nodes: drawn.nodes.slice(0, 1) })).toEqual({ x: 110, y: 120 });
  });
  it('a node is created in the space of the frame as it is drawn', () => {
    const i: CreateIntent = { ...gesture, tool: 'rect', at: 'f', x: 150, y: 170, w: 0, h: 0 };
    expect(createActionFor(i, drawnNodes)).toEqual({ type: 'createNode', kind: 'rectangle', x: 140, y: 150, parent: 'f' });
    expect(createActionFor(i, drawnNodes, drawn)).toEqual({ type: 'createNode', kind: 'rectangle', x: 40, y: 50, parent: 'f' });
  });
  it('a resize starts from the drawn box and sends the drawn value of the side that did not change', () => {
    const rz = (over: Partial<ResizeIntent>): ResizeIntent => ({ ...gesture, ids: ['c'], handle: 'e', dx: 10, dy: 7, ...over });
    // Only the width changes: the height is sent as it is drawn (30), so a hug height is not made fixed by the app layer.
    expect(resizeActionFor(rz({}), drawnNodes, drawn)).toEqual({ type: 'resize', id: 'c', w: 50, h: 30 });
    expect(resizeActionFor(rz({ handle: 's' }), drawnNodes, drawn)).toEqual({ type: 'resize', id: 'c', w: 40, h: 37 });
    expect(resizeActionFor(rz({ handle: 'nw', dx: -4, dy: -2 }), drawnNodes, drawn)).toEqual({ type: 'resize', id: 'c', w: 44, h: 32, x: 1, y: 4 });
  });
  it('a resize of a node the scene does not hold works on the stored box', () => {
    const rz: ResizeIntent = { ...gesture, ids: ['c'], handle: 'e', dx: 10, dy: 0 };
    expect(resizeActionFor(rz, drawnNodes, { ...drawn, nodes: [] })).toEqual({ type: 'resize', id: 'c', w: 11, h: 1 });
  });
  it('the preview of a resize is the drawn box moved by the handle', () => {
    const rz: ResizeIntent = { ...gesture, final: false, ids: ['c'], handle: 'e', dx: 10, dy: 0 };
    expect(resizePreviewFor(rz, drawnNodes, drawn)).toEqual({ kind: 'box', id: 'c', box: { x: 5, y: 6, w: 50, h: 30 } });
    expect(resizePreviewFor({ ...rz, final: true }, drawnNodes, drawn)).toBeNull();
    expect(resizePreviewFor({ ...rz, cancelled: true }, drawnNodes, drawn)).toBeNull();
    expect(resizePreviewFor({ ...rz, ids: ['c', 'f'] }, drawnNodes, drawn)).toBeNull();
    expect(resizePreviewFor({ ...rz, ids: ['ghost'] }, drawnNodes, drawn)).toBeNull();
  });
});

describe('with the flag off the drawn box is the stored box and nothing changes', () => {
  const nodes = [node('f', { type: 'frame', x: 10, y: 20, w: 200, h: 100 }), node('c', { parentId: 'f', x: 3, y: 4, w: 40, h: 30 }), node('t', { parentId: 'f', type: 'text', x: 50, y: 60, w: 70, h: 20 })];
  const scene = storedScene(nodes);
  it('every origin is the same', () => {
    for (const id of [null, 'f', 'c', 't', 'ghost']) expect(worldOrigin(nodes, id, scene)).toEqual(worldOrigin(nodes, id));
  });
  it('every resize, from every handle, is the same action and the same preview', () => {
    const handles = ['nw', 'n', 'ne', 'e', 'se', 's', 'sw', 'w'] as const;
    for (const id of ['f', 'c', 't']) {
      for (const handle of handles) {
        for (const [dx, dy] of [[10, 5], [-7, -3], [-500, 500]] as const) {
          const i: ResizeIntent = { ...gesture, ids: [id], handle, dx, dy };
          expect(JSON.stringify(resizeActionFor(i, nodes, scene))).toBe(JSON.stringify(resizeActionFor(i, nodes)));
          const live = { ...i, final: false };
          expect(JSON.stringify(resizePreviewFor(live, nodes, scene))).toBe(JSON.stringify(resizePreviewFor(live, nodes)));
        }
      }
    }
  });
  it('every created node is the same', () => {
    for (const at of [null, 'f', 'c']) {
      const i: CreateIntent = { ...gesture, tool: 'text', at, x: 77, y: 88, w: 30, h: 20 };
      expect(JSON.stringify(createActionFor(i, nodes, scene))).toBe(JSON.stringify(createActionFor(i, nodes)));
    }
  });
});

describe('the preview of a drag', () => {
  const nodes = [node('a'), node('b'), node('f', { type: 'frame' }), node('c', { parentId: 'f' }), node('g', { type: 'frame', parentId: 'f' }), node('d', { parentId: 'g' })];
  const m = (over: Partial<MoveIntent> = {}): MoveIntent => ({ ...gesture, final: false, ids: ['a'], dx: 5, dy: 6, at: { x: 7, y: 8 }, ...over });
  const slot = { frame: 'f', index: 1, line: { x: 1, y: 2, w: 30, h: 0 } };
  it('shows the line of the slot the app layer names, and the dragged box stays where it is drawn', () => {
    const asked: unknown[][] = [];
    const p = movePreviewFor(m(), nodes, (id, at, by) => (asked.push([id, at, by]), slot));
    expect(p).toEqual({ kind: 'slot', line: slot.line });
    expect(asked).toEqual([['a', { x: 7, y: 8 }, { dx: 5, dy: 6 }]]);
    const scene: Scene = { space: 'parent', nodes: [{ id: 'a', parent: null, kind: 'rect', x: 1, y: 1, w: 5, h: 5, fill: 1 }] };
    expect(previewScene(scene, p)).toBe(scene);
  });
  it('shows the box moving with the pointer when there is no slot', () => {
    expect(movePreviewFor(m(), nodes, () => null)).toEqual({ kind: 'move', ids: ['a'], dx: 5, dy: 6 });
  });
  it('asks about the one node that the drop is of, as moveActionsFor does: a node and its selected descendants are one dragged node', () => {
    for (const ids of [['f', 'c'], ['f', 'd', 'c'], ['c', 'f']]) {
      const asked: string[] = [];
      expect(movePreviewFor(m({ ids }), nodes, (id) => (asked.push(id), slot))).toEqual({ kind: 'slot', line: slot.line });
      expect(asked).toEqual(['f']);
      // The same intent, finished, is a drop of that node.
      expect(moveActionsFor(m({ ids, final: true }), ids, nodes)).toEqual([{ type: 'dropNode', id: 'f', x: 7, y: 8, dx: 5, dy: 6 }]);
    }
  });
  it('asks nothing for nodes that are not one dragged node (a selection of several: moveBy), and shows them moving', () => {
    let asked = 0;
    expect(movePreviewFor(m({ ids: ['a', 'b'] }), nodes, () => (asked++, slot))).toEqual({ kind: 'move', ids: ['a', 'b'], dx: 5, dy: 6 });
    expect(movePreviewFor(m({ ids: ['c', 'g'] }), nodes, () => (asked++, slot))).toEqual({ kind: 'move', ids: ['c', 'g'], dx: 5, dy: 6 });
    expect(asked).toBe(0);
    expect(moveActionsFor(m({ ids: ['a', 'b'], final: true }), ['a', 'b'], nodes)).toEqual([{ type: 'moveBy', dx: 5, dy: 6 }]);
  });
  it('shows nothing when the gesture ended or was cancelled', () => {
    expect(movePreviewFor(m({ final: true }), nodes, () => slot)).toBeNull();
    expect(movePreviewFor(m({ cancelled: true }), nodes, () => slot)).toBeNull();
  });
});

describe('deleteAction', () => {
  it('deletes the whole selection in one action, and nothing when nothing is selected', () => {
    expect(deleteAction(['a', 'b'])).toEqual({ type: 'delete', ids: ['a', 'b'] });
    expect(deleteAction([])).toBeNull();
  });
  it('is the rule of the Delete and Backspace keys', () => {
    for (const key of ['Delete', 'Backspace']) {
      expect(keyAction({ key, ctrlKey: false, metaKey: false, altKey: false, shiftKey: false }, ['a'])).toEqual(deleteAction(['a']));
      expect(keyAction({ key, ctrlKey: false, metaKey: false, altKey: false, shiftKey: false }, [])).toEqual(deleteAction([]));
    }
  });
  it('copies the ids, so a later change of the selection does not change the action', () => {
    const sel = ['a'];
    const a = deleteAction(sel);
    sel.push('b');
    expect(a).toEqual({ type: 'delete', ids: ['a'] });
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

describe('ownsKeys', () => {
  // `inside` lists what the element sits in, as the selectors that would match it: the fake answers `closest` with the real selector.
  const el = (tagName: string, inside: string[] = [], isContentEditable = false) => ({
    tagName,
    isContentEditable,
    closest: (sel: string) => (sel.split(',').map((x) => x.trim()).some((alt) => inside.includes(alt)) ? {} : null),
  });
  const card = ['[data-owns-keys]'];
  it('leaves the keys to a field that takes text', () => {
    for (const tag of ['INPUT', 'TEXTAREA', 'SELECT']) expect(ownsKeys(el(tag))).toBe(true);
    expect(ownsKeys(el('DIV', [], true))).toBe(true);
  });
  it('names the marker of the layout and the roles of a menu and a dialog', () => {
    expect(OWNS_KEYS_SELECTOR).toBe('[data-owns-keys], [role=menu], [role=dialog]');
  });
  it('leaves the plain keys to a button inside a card of the layout, an item of a menu or a dialog', () => {
    expect(ownsKeys(el('BUTTON', card))).toBe(true);
    expect(ownsKeys(el('DIV', ['[role=menu]']))).toBe(true);
    expect(ownsKeys(el('BUTTON', ['[role=dialog]']))).toBe(true);
  });
  it('still lets undo, redo and Option+1 through from a button of a card, but not from a field', () => {
    expect(ownsKeys(el('BUTTON', card), { ctrlKey: true, metaKey: false })).toBe(false);
    expect(ownsKeys(el('BUTTON', card), { ctrlKey: false, metaKey: true })).toBe(false);
    expect(ownsKeys(el('BUTTON', card), { ctrlKey: false, metaKey: false, altKey: true })).toBe(false);
    expect(ownsKeys(el('INPUT', card), { ctrlKey: true, metaKey: false })).toBe(true);
  });
  it('keeps the keys of the editor for the canvas, the page and the tool bar', () => {
    expect(ownsKeys(el('CANVAS'))).toBe(false);
    expect(ownsKeys(el('BUTTON'))).toBe(false);
    expect(ownsKeys(el('BUTTON', ['aside']))).toBe(false);
    expect(ownsKeys(null)).toBe(false);
  });
});
