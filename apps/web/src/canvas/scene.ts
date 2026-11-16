// The scene the canvas draws, as plain data, and the pure functions on it. No Pixi here.
//
// Coordinate choice. A node carries x, y, w, h in the space named by `Scene.space`:
//  - 'parent': x and y are relative to the parent node (the document model's own form);
//  - 'world': x and y are already resolved world coordinates (the form the layout rects of the WASM
//    engine will arrive in).
// Either way `resolveScene` produces one resolved form with both the world rect and the offset in the
// parent, so the drawing code and the hit test never care which one came in. Array order is paint
// order among siblings (first is at the back); a parent is painted before its children.
//
// The canvas draws what it is given: no layout, no editor rules (clamping, snapping, constraints) live here.
import { containsPoint, intersectRects, rectsEqual, type Rect } from './geometry';

export type NodeKind = 'frame' | 'rect' | 'text';
export type SceneSpace = 'parent' | 'world';

export interface SceneNode {
  id: string;
  /** Parent node id; null for a node on the canvas itself. */
  parent: string | null;
  kind: NodeKind;
  x: number;
  y: number;
  w: number;
  h: number;
  /** `0xRRGGBB` of the node's own fill (document data, not a UI token); null for no fill. */
  fill: number | null;
  /** Corner radius in world units. */
  radius?: number;
  /** Text content (kind 'text'). */
  text?: string;
  /** Font size in world units (kind 'text'). */
  fontSize?: number;
  /** Name drawn above a frame that sits on the canvas. */
  name?: string;
  /** A frame that clips its children to its own rectangle. */
  clip?: boolean;
  /** False hides the node and its whole subtree; absent means visible. */
  visible?: boolean;
  /** 0 to 1; absent means 1. */
  opacity?: number;
}

export interface Scene {
  space: SceneSpace;
  nodes: readonly SceneNode[];
}

/** A node with everything the canvas needs resolved. Objects are shared between resolutions while unchanged. */
export interface ResolvedNode {
  id: string;
  parent: string | null;
  kind: NodeKind;
  /** 0 for a node on the canvas. */
  depth: number;
  /** World rectangle. */
  x: number;
  y: number;
  w: number;
  h: number;
  /** Offset inside the parent (equal to x, y on the canvas). */
  lx: number;
  ly: number;
  fill: number | null;
  radius: number;
  text: string;
  fontSize: number;
  name: string;
  clip: boolean;
  /** Visible itself and through every ancestor. */
  visible: boolean;
  opacity: number;
  /** The intersection of the clipping ancestors' rectangles; null when nothing clips it. */
  clipRect: Rect | null;
}

export interface ResolvedScene {
  /** Paint order, back to front: a parent before its children. */
  items: readonly ResolvedNode[];
  byId: ReadonlyMap<string, ResolvedNode>;
}

export interface ScenePatch {
  added: string[];
  removed: string[];
  /** Present before and after, with any resolved field different. */
  changed: string[];
  /** The paint order differs from before (a node was added, removed or moved in the order). */
  orderChanged: boolean;
}

export const EMPTY_SCENE: ResolvedScene = { items: [], byId: new Map() };

function sameNode(a: ResolvedNode, b: ResolvedNode): boolean {
  return (
    a.parent === b.parent && a.kind === b.kind && a.depth === b.depth &&
    rectsEqual(a, b) && a.lx === b.lx && a.ly === b.ly &&
    a.fill === b.fill && a.radius === b.radius && a.text === b.text && a.fontSize === b.fontSize &&
    a.name === b.name && a.clip === b.clip && a.visible === b.visible && a.opacity === b.opacity &&
    (a.clipRect === b.clipRect || (!!a.clipRect && !!b.clipRect && rectsEqual(a.clipRect, b.clipRect)))
  );
}

function resolveAll(scene: Scene): ResolvedNode[] {
  const kids = new Map<string | null, SceneNode[]>();
  const ids = new Set<string>();
  for (const n of scene.nodes) {
    if (ids.has(n.id)) continue; // a repeated id: the first one wins
    ids.add(n.id);
    const list = kids.get(n.parent);
    if (list) list.push(n);
    else kids.set(n.parent, [n]);
  }
  const out: ResolvedNode[] = [];
  const visit = (parent: ResolvedNode | null, parentId: string | null) => {
    for (const n of kids.get(parentId) ?? []) {
      const wx = scene.space === 'world' || !parent ? n.x : parent.x + n.x;
      const wy = scene.space === 'world' || !parent ? n.y : parent.y + n.y;
      const own: Rect = { x: wx, y: wy, w: n.w, h: n.h };
      const parentClip = parent
        ? parent.clip ? (parent.clipRect ? intersectRects(parent.clipRect, parent) : { x: parent.x, y: parent.y, w: parent.w, h: parent.h }) : parent.clipRect
        : null;
      const node: ResolvedNode = {
        id: n.id,
        parent: n.parent,
        kind: n.kind,
        depth: parent ? parent.depth + 1 : 0,
        ...own,
        lx: parent ? wx - parent.x : wx,
        ly: parent ? wy - parent.y : wy,
        fill: n.fill,
        radius: n.radius ?? 0,
        text: n.text ?? '',
        fontSize: n.fontSize ?? 0,
        name: n.name ?? '',
        clip: n.clip ?? false,
        visible: (n.visible ?? true) && (parent ? parent.visible : true),
        opacity: n.opacity ?? 1,
        clipRect: parentClip,
      };
      out.push(node);
      visit(node, n.id);
    }
  };
  visit(null, null);
  return out;
}

/**
 * Resolves `scene` and compares it with `prev`. Nodes that did not change keep the object of `prev`, so
 * the drawing code can skip them by identity; the patch lists what to add, remove and update. A node whose
 * parent is missing or that sits in a cycle is not part of the result.
 */
export function resolveScene(scene: Scene, prev: ResolvedScene = EMPTY_SCENE): { scene: ResolvedScene; patch: ScenePatch } {
  const fresh = resolveAll(scene);
  const items: ResolvedNode[] = [];
  const byId = new Map<string, ResolvedNode>();
  const patch: ScenePatch = { added: [], removed: [], changed: [], orderChanged: fresh.length !== prev.items.length };
  fresh.forEach((n, i) => {
    const old = prev.byId.get(n.id);
    let keep = n;
    if (!old) patch.added.push(n.id);
    else if (sameNode(old, n)) keep = old;
    else patch.changed.push(n.id);
    if (prev.items[i]?.id !== n.id) patch.orderChanged = true;
    items.push(keep);
    byId.set(n.id, keep);
  });
  for (const o of prev.items) if (!byId.has(o.id)) patch.removed.push(o.id);
  return { scene: { items, byId }, patch };
}

/** The front-most visible node under the world point, or null. Clipped-away parts do not hit. */
export function hitTest(rs: ResolvedScene, x: number, y: number, slop = 0): string | null {
  for (let i = rs.items.length - 1; i >= 0; i--) {
    const n = rs.items[i]!;
    if (!n.visible) continue;
    if (n.clipRect && !containsPoint(n.clipRect, x, y)) continue;
    if (
      x >= n.x - slop && x <= n.x + n.w + slop && y >= n.y - slop && y <= n.y + n.h + slop
    ) return n.id;
  }
  return null;
}

/** World rectangles of the given ids (unknown ids are skipped). */
export function rectsOf(rs: ResolvedScene, ids: readonly string[]): Rect[] {
  const out: Rect[] = [];
  for (const id of ids) {
    const n = rs.byId.get(id);
    if (n) out.push({ x: n.x, y: n.y, w: n.w, h: n.h });
  }
  return out;
}

/** The rectangle around every visible node on the canvas, or null for an empty scene. */
export function contentBounds(rs: ResolvedScene): Rect | null {
  let out: Rect | null = null;
  for (const n of rs.items) {
    if (n.depth !== 0 || !n.visible) continue;
    const r = { x: n.x, y: n.y, w: n.w, h: n.h };
    if (!out) out = r;
    else {
      const x = Math.min(out.x, r.x);
      const y = Math.min(out.y, r.y);
      out = { x, y, w: Math.max(out.x + out.w, r.x + r.w) - x, h: Math.max(out.y + out.h, r.y + r.h) - y };
    }
  }
  return out;
}
