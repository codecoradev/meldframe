// What the canvas reports (a move, a resize, a drawn box) as actions of the app layer, and what to show while a
// gesture is going on. Pure: the canvas applies nothing and this module holds no state. The app layer decides.
import type { Action, CreateKind, Node, Tool } from '@meldframe/protocol';
import type { CreateIntent, CreateTool, MoveIntent, ResizeIntent, ToolMode } from '@meldframe/web-canvas/interaction';
import type { HandleId } from '@meldframe/web-canvas/overlay';
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

/** Where the space of a node starts on the canvas: the origins along its parents added up (the canvas itself is 0, 0). */
export function worldOrigin(nodes: readonly Node[], id: string | null): { x: number; y: number } {
  if (!cycleFree(nodes, id)) return { x: 0, y: 0 }; // a file with a cycle is not trusted for positions
  let x = 0;
  let y = 0;
  for (let n = byId(nodes, id); n; n = byId(nodes, n.parentId)) {
    x += n.x;
    y += n.y;
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

/** A drawn box (or a click) as the action that makes the node, in the space of the frame it goes into. */
export function createActionFor(i: CreateIntent, nodes: readonly Node[]): Action | null {
  if (!i.final || i.cancelled) return null;
  const parent = containerFor(i.at, nodes);
  const o = worldOrigin(nodes, parent);
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

/**
 * A finished move as actions: the whole selection moves as one step; anything else node by node. A node and a descendant
 * of it both being selected, only the node moves (positions are relative to the parent: the descendant would move twice).
 */
export function moveActionsFor(i: MoveIntent, selection: readonly string[], nodes: readonly Node[]): Action[] {
  if (!i.final || i.cancelled || (i.dx === 0 && i.dy === 0)) return [];
  const tops = topOf(i.ids, (id) => byId(nodes, id)?.parentId ?? null);
  const same = tops.length === i.ids.length && i.ids.length === selection.length && i.ids.every((id) => selection.includes(id));
  if (same) return [{ type: 'moveBy', dx: i.dx, dy: i.dy }];
  return tops.flatMap((id) => {
    const n = byId(nodes, id);
    return n ? [{ type: 'move', id, x: n.x + i.dx, y: n.y + i.dy } as Action] : [];
  });
}

/** A finished resize of one node as one action (the origin goes with it when the left or top edge moved). */
export function resizeActionFor(i: ResizeIntent, nodes: readonly Node[]): Action | null {
  if (!i.final || i.cancelled || i.ids.length !== 1) return null;
  const n = byId(nodes, i.ids[0]!);
  if (!n) return null;
  const b = resizedBox(n, i.handle, i.dx, i.dy);
  if (b.x === n.x && b.y === n.y && b.w === n.w && b.h === n.h) return null;
  return { type: 'resize', id: n.id, w: b.w, h: b.h, ...(b.x !== n.x ? { x: b.x } : {}), ...(b.y !== n.y ? { y: b.y } : {}) };
}

/** What is shown while a gesture is going on; the file changes only when it ends. */
export type Preview = { kind: 'move'; ids: readonly string[]; dx: number; dy: number } | { kind: 'box'; id: string; box: Box };

/** The scene with the preview applied: the moved nodes (their descendants follow) or a resized box. (The box being drawn is the canvas's own.) */
export function previewScene(scene: Scene, p: Preview | null): Scene {
  if (!p) return scene;
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

/**
 * Whether a key pressed with focus on this element belongs to the element, not to the editor: a field that takes text
 * keeps every key; a button or anything else in a side panel keeps the plain keys (Enter activates it, Delete and the
 * tool letters must not reach the canvas), while Ctrl or Cmd combinations such as undo still go to the editor.
 */
export function ownsKeys(el: KeyTarget | null, k: { ctrlKey: boolean; metaKey: boolean } = { ctrlKey: false, metaKey: false }): boolean {
  if (!el) return false;
  if (el.isContentEditable || /^(input|textarea|select)$/i.test(el.tagName)) return true;
  return el.closest('aside') !== null && !k.ctrlKey && !k.metaKey;
}

/** The action of a key of the editor: tools, delete, undo and redo. Null for any other key. */
export function keyAction(k: KeyLike, selection: readonly string[]): Action | null {
  const mod = k.metaKey || k.ctrlKey;
  const key = k.key.toLowerCase();
  if (!mod && !k.altKey && toolKeys[key]) return { type: 'setTool', tool: toolKeys[key] };
  if (!mod && (k.key === 'Delete' || k.key === 'Backspace')) return selection.length ? { type: 'delete', ids: [...selection] } : null;
  if (mod && !k.altKey && key === 'z') return k.shiftKey ? { type: 'redo' } : { type: 'undo' };
  if (mod && !k.altKey && !k.shiftKey && key === 'y') return { type: 'redo' };
  return null;
}

const modes: Record<Tool, ToolMode> = { select: 'move', frame: 'frame', rectangle: 'rect', text: 'text' };
/** The tool of the app as the mode of the canvas. */
export const toolMode = (t: Tool): ToolMode => modes[t];
