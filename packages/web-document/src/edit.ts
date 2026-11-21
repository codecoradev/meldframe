// What the canvas reports (a move, a resize, a drawn box) as actions of the app layer, and what to show while a
// gesture is going on. Pure: the canvas applies nothing and this module holds no state. The app layer decides.
import type { Action, CreateKind, Direction, Node, Tool } from '@meldframe/protocol';
import type { CreateIntent, CreateTool, MoveIntent, ResizeIntent, ToolMode } from '@meldframe/web-canvas/interaction';
import type { DropSlot, HandleId } from '@meldframe/web-canvas/overlay';
import type { Scene } from '@meldframe/web-canvas/scene';

export interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** The smallest size a resize leaves a node (a node of no size cannot be grabbed again). */
export const MIN_SIZE = 1;

/** The box after a handle moved by (dx, dy): the edge the handle holds moves, the opposite one stays. */
export function resizedBox(b: Box, handle: HandleId, dx: number, dy: number): Box {
  let { x, y, w, h } = b;
  if (handle.includes('e')) w = b.w + dx;
  if (handle.includes('w')) {
    x = b.x + dx;
    w = b.w - dx;
  }
  if (handle.includes('s')) h = b.h + dy;
  if (handle.includes('n')) {
    y = b.y + dy;
    h = b.h - dy;
  }
  if (w < MIN_SIZE) {
    if (handle.includes('w')) x = b.x + b.w - MIN_SIZE;
    w = MIN_SIZE;
  }
  if (h < MIN_SIZE) {
    if (handle.includes('n')) y = b.y + b.h - MIN_SIZE;
    h = MIN_SIZE;
  }
  return { x, y, w, h };
}

const byId = (nodes: readonly Node[], id: string | null) => (id === null ? undefined : nodes.find((n) => n.id === id));

/**
 * The box the canvas draws a node at, in the space of its parent, read from the scene the engine hands over; null when the
 * scene has no such node. With the host flag `layout_on_canvas` off it is the stored box; with it on, the laid-out one
 * (hug, fill, auto layout), which is what the person sees and points at. Only a scene in parent space is read.
 */
export function drawnBox(scene: Scene | undefined, id: string): Box | null {
  if (!scene || scene.space !== 'parent') return null;
  const n = scene.nodes.find((x) => x.id === id);
  return n ? { x: n.x, y: n.y, w: n.w, h: n.h } : null;
}

/**
 * Where the space of a node starts on the canvas: the origins along its parents added up (the canvas itself is 0, 0). With
 * the `scene` each origin is the one the node is drawn at, else the stored one.
 */
export function worldOrigin(nodes: readonly Node[], id: string | null, scene?: Scene): { x: number; y: number } {
  if (!cycleFree(nodes, id)) return { x: 0, y: 0 }; // a file with a cycle is not trusted for positions
  let x = 0;
  let y = 0;
  for (let n = byId(nodes, id); n; n = byId(nodes, n.parentId)) {
    const b = drawnBox(scene, n.id) ?? n;
    x += b.x;
    y += b.y;
  }
  return { x, y };
}

/** True when the chain of parents from `id` ends. */
function cycleFree(nodes: readonly Node[], id: string | null): boolean {
  const seen = new Set<string>();
  for (let n = byId(nodes, id); n; n = byId(nodes, n.parentId)) {
    if (seen.has(n.id)) return false;
    seen.add(n.id);
  }
  return true;
}

/** The frame a new node goes into: the node under the press if it is a frame, else the nearest frame above it, else the canvas. */
export function containerFor(at: string | null, nodes: readonly Node[]): string | null {
  const seen = new Set<string>();
  for (let n = byId(nodes, at); n && !seen.has(n.id); n = byId(nodes, n.parentId)) {
    if (n.type === 'frame') return n.id;
    seen.add(n.id);
  }
  return null;
}

const kindOf: Record<CreateTool, CreateKind> = { frame: 'frame', rect: 'rectangle', text: 'text' };

/** A drawn box (or a click) as the action that makes the node, in the space of the frame it goes into (as it is drawn, with the `scene`). */
export function createActionFor(i: CreateIntent, nodes: readonly Node[], scene?: Scene): Action | null {
  if (!i.final || i.cancelled) return null;
  const parent = containerFor(i.at, nodes);
  const o = worldOrigin(nodes, parent, scene);
  const at = { x: i.x - o.x, y: i.y - o.y };
  // The canvas reports a click as a box of size 0 and starts a drag only after a few pixels: a box with a size was drawn.
  if (i.w === 0 && i.h === 0) return { type: 'createNode', kind: kindOf[i.tool], ...at, parent };
  // A box dragged flat in one direction still gets a size that can be grabbed.
  return { type: 'createNode', kind: kindOf[i.tool], ...at, parent, w: Math.max(i.w, MIN_SIZE), h: Math.max(i.h, MIN_SIZE) };
}

/** The nodes that move on their own: those without a moved ancestor (a descendant follows its parent). */
function topOf(ids: readonly string[], parentOf: (id: string) => string | null): string[] {
  const moved = new Set(ids);
  return ids.filter((id) => {
    const seen = new Set<string>();
    for (let p = parentOf(id); p !== null && !seen.has(p); p = parentOf(p)) {
      if (moved.has(p)) return false;
      seen.add(p);
    }
    return true;
  });
}

/** The top nodes of the dragged ids: the one rule of "which node is dragged" for the drop and for its preview. */
const topNodes = (ids: readonly string[], nodes: readonly Node[]) => topOf(ids, (id) => byId(nodes, id)?.parentId ?? null);

/**
 * A finished move as actions: the whole selection moves as one step; one dragged node (with its selected descendants, which
 * follow it) is dropped where the pointer is, and the app layer decides whether that is a move, a reorder in a flow or a
 * reparent (with the layout off it is the plain move); anything else moves node by node. A node and a descendant of it both
 * being selected, only the node moves (positions are relative to the parent: the descendant would move twice).
 * The layout rules are those of one dropped node: a selection of several is moved by the same distance, whatever the layout
 * (`moveBy`; the app layer does not guard it), so the children of a flow in it change their stored x and y and not the flow.
 */
export function moveActionsFor(i: MoveIntent, selection: readonly string[], nodes: readonly Node[]): Action[] {
  if (!i.final || i.cancelled || (i.dx === 0 && i.dy === 0)) return [];
  const tops = topNodes(i.ids, nodes);
  if (tops.length === 1) {
    const n = byId(nodes, tops[0]!);
    return n ? [{ type: 'dropNode', id: n.id, x: i.at.x, y: i.at.y, dx: i.dx, dy: i.dy }] : [];
  }
  const same = tops.length === i.ids.length && i.ids.length === selection.length && i.ids.every((id) => selection.includes(id));
  if (same) return [{ type: 'moveBy', dx: i.dx, dy: i.dy }];
  return tops.flatMap((id) => {
    const n = byId(nodes, id);
    return n ? [{ type: 'move', id, x: n.x + i.dx, y: n.y + i.dy } as Action] : [];
  });
}

/**
 * A finished resize of one node as one action (the origin goes with it when the left or top edge moved). It starts from the
 * box the node is drawn at (the `scene`; the stored box when the scene has none), and the side the handle does not touch is
 * sent as it is drawn, so the app layer does not take it for a change and make a hug or fill side fixed.
 */
export function resizeActionFor(i: ResizeIntent, nodes: readonly Node[], scene?: Scene): Action | null {
  if (!i.final || i.cancelled || i.ids.length !== 1) return null;
  const n = byId(nodes, i.ids[0]!);
  if (!n) return null;
  const from = drawnBox(scene, n.id) ?? n;
  const b = resizedBox(from, i.handle, i.dx, i.dy);
  if (b.x === from.x && b.y === from.y && b.w === from.w && b.h === from.h) return null;
  return { type: 'resize', id: n.id, w: b.w, h: b.h, ...(b.x !== from.x ? { x: b.x } : {}), ...(b.y !== from.y ? { y: b.y } : {}) };
}

/** What is shown while a gesture is going on; the file changes only when it ends. */
export type Preview =
  | { kind: 'move'; ids: readonly string[]; dx: number; dy: number }
  | { kind: 'box'; id: string; box: Box }
  // Where in a flow the dragged node would go: a line over the canvas (the box itself stays where it is drawn).
  | { kind: 'slot'; line: Box };

/**
 * What a drag shows: the slot the app layer names for the one node that is dragged (the same rule as `moveActionsFor`: a node
 * and its selected descendants are one node), else the boxes moving with the pointer.
 */
export function movePreviewFor(i: MoveIntent, nodes: readonly Node[], slotOf: (id: string, at: { x: number; y: number }, by: { dx: number; dy: number }) => DropSlot | null): Preview | null {
  if (i.cancelled || i.final) return null;
  const tops = topNodes(i.ids, nodes);
  const slot = tops.length === 1 ? slotOf(tops[0]!, i.at, { dx: i.dx, dy: i.dy }) : null;
  return slot ? { kind: 'slot', line: slot.line } : { kind: 'move', ids: i.ids, dx: i.dx, dy: i.dy };
}

/** What a handle drag shows: the box it is drawn at, moved by the handle (not the stored one). */
export function resizePreviewFor(i: ResizeIntent, nodes: readonly Node[], scene?: Scene): Preview | null {
  if (i.cancelled || i.final || i.ids.length !== 1) return null;
  const n = byId(nodes, i.ids[0]!);
  return n ? { kind: 'box', id: n.id, box: resizedBox(drawnBox(scene, n.id) ?? n, i.handle, i.dx, i.dy) } : null;
}

/** The scene with the preview applied: the moved nodes (their descendants follow) or a resized box. A slot changes no box, and the box being drawn is the canvas's own. */
export function previewScene(scene: Scene, p: Preview | null): Scene {
  if (!p || p.kind === 'slot') return scene;
  const parents = new Map(scene.nodes.map((n) => [n.id, n.parent]));
  const tops = p.kind === 'move' ? new Set(topOf(p.ids, (id) => parents.get(id) ?? null)) : null;
  return {
    ...scene,
    nodes: scene.nodes.map((n) => {
      if (p.kind === 'move') return tops!.has(n.id) ? { ...n, x: n.x + p.dx, y: n.y + p.dy } : n;
      return n.id === p.id ? { ...n, ...p.box } : n;
    }),
  };
}

export interface KeyLike {
  key: string;
  metaKey: boolean;
  ctrlKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
}

const toolKeys: Record<string, Tool> = { v: 'select', f: 'frame', r: 'rectangle', t: 'text' };

/** The part of an element that `ownsKeys` reads. */
export interface KeyTarget {
  tagName: string;
  isContentEditable: boolean;
  closest(selector: string): unknown;
}

/** The part of an element that `isControl` reads. */
export interface ControlTarget {
  tagName: string;
  getAttribute(name: string): string | null;
}

/** A control that the Enter key activates (a button, a link, a menu item, a tab, a radio): Enter belongs to it, not to the editor. */
export function isControl(el: ControlTarget | null): boolean {
  if (!el) return false;
  if (/^(button|a|summary|select|input|textarea)$/i.test(el.tagName)) return true;
  return /^(button|link|menuitem|tab|radio|checkbox|switch|option)$/.test(el.getAttribute('role') ?? '');
}

/** What marks a place that keeps the plain keys for what is inside it: the cards of the layout, a menu, a dialog. */
export const OWNS_KEYS_SELECTOR = '[data-owns-keys], [role=menu], [role=dialog]';

/**
 * Whether a key pressed with focus on this element belongs to the element, not to the editor: a field that takes text
 * keeps every key; a button or anything else inside a card of the layout, a menu or a dialog keeps the plain keys (Enter
 * activates it, Delete and the tool letters must not reach the canvas, typing in a menu is a search). Ctrl and Cmd
 * combinations such as undo, and Alt combinations such as Option+1, still go to the editor.
 */
export function ownsKeys(el: KeyTarget | null, k: { ctrlKey: boolean; metaKey: boolean; altKey?: boolean } = { ctrlKey: false, metaKey: false }): boolean {
  if (!el) return false;
  if (el.isContentEditable || /^(input|textarea|select)$/i.test(el.tagName)) return true;
  return el.closest(OWNS_KEYS_SELECTOR) !== null && !k.ctrlKey && !k.metaKey && !k.altKey;
}

/** The action that deletes the selection (one action for all of it), or null when nothing is selected: the rule of the Delete key and of the palette. */
export const deleteAction = (selection: readonly string[]): Action | null => (selection.length ? { type: 'delete', ids: [...selection] } : null);

/** Whether a dispatch changed the document: the snapshot is a new one and the app layer raised `documentChanged` for it. */
export function documentChanged(before: { events: readonly { type: string }[] }, after: { events: readonly { type: string }[] }): boolean {
  return after !== before && after.events.some((e) => e.type === 'documentChanged');
}

/** Whether a gesture of the canvas (a drag, a resize, a drawn box) is still going on: its last intent has not come. */
export const gestureGoing = (i: { final: boolean; cancelled: boolean }): boolean => !i.final && !i.cancelled;

const arrowKeys: Record<string, Direction> = { ArrowUp: 'up', ArrowDown: 'down', ArrowLeft: 'left', ArrowRight: 'right' };

/**
 * The action of a key of the editor: tools, delete, undo and redo, and a plain arrow key with a selection (the app layer
 * decides what it does: with the layout on the canvas it moves the selected children of an auto layout frame one place, and
 * it does nothing otherwise). Null for any other key.
 */
export function keyAction(k: KeyLike, selection: readonly string[]): Action | null {
  const mod = k.metaKey || k.ctrlKey;
  const key = k.key.toLowerCase();
  if (!mod && !k.altKey && toolKeys[key]) return { type: 'setTool', tool: toolKeys[key] };
  if (!mod && (k.key === 'Delete' || k.key === 'Backspace')) return deleteAction(selection);
  if (mod && !k.altKey && key === 'z') return k.shiftKey ? { type: 'redo' } : { type: 'undo' };
  if (mod && !k.altKey && !k.shiftKey && key === 'y') return { type: 'redo' };
  const arrow = Object.hasOwn(arrowKeys, k.key) ? arrowKeys[k.key] : undefined;
  if (arrow && !mod && !k.altKey && !k.shiftKey && selection.length) return { type: 'arrowKey', direction: arrow };
  return null;
}

const modes: Record<Tool, ToolMode> = { select: 'move', frame: 'frame', rectangle: 'rect', text: 'text' };
/** The tool of the app as the mode of the canvas. */
export const toolMode = (t: Tool): ToolMode => modes[t];
