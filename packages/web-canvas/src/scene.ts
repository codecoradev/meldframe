// The scene the canvas draws, as plain data, and the pure functions on it. No Pixi here.
//
// Coordinate choice. A node carries x, y, w, h in the space named by `Scene.space`:
//  - 'parent': x and y are relative to the parent node (the document model's own form);
//  - 'world': x and y are already resolved world coordinates (the form the layout rects of the WASM
//    engine will arrive in).
// Either way `resolveScene` produces one resolved form with both the world rect and the offset in the
// parent, so the drawing code never cares which one came in. What is under a point is not decided here: the app layer
// answers it (`meldframe_render::HitIndex`, asked through the host, T9), so a click and a drop read one rule. Array order is paint
// order among siblings (first is at the back); a parent is painted before its children.
//
// The canvas draws what it is given: no layout, no editor rules (clamping, snapping, constraints) live here.
import { intersectRects, rectsEqual, unionRects, type Rect } from './geometry';
import { sameBorder, type SceneBorder } from '@meldframe/web-paint/border';
import { sameGradient, type SceneGradient } from '@meldframe/web-paint/gradient';
import { sameImage, type SceneImage } from '@meldframe/web-paint/image';
import { sameShadows, type SceneShadow } from '@meldframe/web-paint/shadow';

export type { SceneBorder, SceneBorderPosition } from '@meldframe/web-paint/border';
export type { SceneGradient } from '@meldframe/web-paint/gradient';
export type { SceneImage, SceneImageFit } from '@meldframe/web-paint/image';
export type { SceneShadow } from '@meldframe/web-paint/shadow';
export type NodeKind = 'frame' | 'rect' | 'text';
export type SceneSpace = 'parent' | 'world';
export type SceneTextAlign = 'left' | 'center' | 'right';

/**
 * How far a node paints beyond its own rectangle, on each side in world units (none below 0): the part of its border that lies
 * outside it and its shadows with their offset, spread and blur. It is worked out once, by `meldframe-render` (`PaintedBox.paint_bounds`),
 * and read here as it is given: the canvas works none of it out.
 */
export interface ScenePaintBounds {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

export function samePaintBounds(a: ScenePaintBounds | undefined, b: ScenePaintBounds | undefined): boolean {
  if (a === b) return true;
  if (!a || !b) return false;
  return a.left === b.left && a.top === b.top && a.right === b.right && a.bottom === b.bottom;
}

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
  /** The alpha of the fill's own colour, 0 to 1 (not the node's `opacity`, which fades the node and what is in it); absent means 1. */
  fillAlpha?: number;
  /** Corner radius in world units. */
  radius?: number;
  /** Text content (kind 'text'). */
  text?: string;
  /** Font size in world units (kind 'text'). */
  fontSize?: number;
  /** 100 to 900 (kind 'text'); absent means 400. */
  fontWeight?: number;
  /** Where the lines sit in the box (kind 'text'); absent means left. */
  textAlign?: SceneTextAlign;
  /** Name drawn above a frame that sits on the canvas. */
  name?: string;
  /** A frame that clips its children to its own rectangle. */
  clip?: boolean;
  /** False hides the node and its whole subtree; absent means visible. */
  visible?: boolean;
  /** 0 to 1; absent means 1. */
  opacity?: number;
  /** Painted over the fill; absent means none. */
  gradient?: SceneGradient;
  /** Painted over the fill and the gradient; absent means none. */
  image?: SceneImage;
  /** Drawn over the node and its children, and not clipped by the node's own `clip`; absent means none. */
  border?: SceneBorder;
  /** Drawn behind the node, first = the bottom of the stack, and not clipped by the node's own `clip`; absent or empty means none. */
  shadows?: readonly SceneShadow[];
  /** What the node paints beyond its rectangle; absent means nothing (its paint stays inside its rectangle). */
  paintBounds?: ScenePaintBounds;
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
  /** The alpha of the fill's own colour, 0 to 1. */
  fillAlpha: number;
  radius: number;
  text: string;
  fontSize: number;
  fontWeight: number;
  textAlign: SceneTextAlign;
  name: string;
  clip: boolean;
  /** Visible itself and through every ancestor. */
  visible: boolean;
  opacity: number;
  gradient: SceneGradient | undefined;
  image: SceneImage | undefined;
  border: SceneBorder | undefined;
  shadows: readonly SceneShadow[] | undefined;
  paintBounds: ScenePaintBounds | undefined;
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
    a.fill === b.fill && a.fillAlpha === b.fillAlpha && a.radius === b.radius && a.text === b.text && a.fontSize === b.fontSize && a.fontWeight === b.fontWeight && a.textAlign === b.textAlign &&
    a.name === b.name && a.clip === b.clip && a.visible === b.visible && a.opacity === b.opacity && sameGradient(a.gradient, b.gradient) && sameImage(a.image, b.image) && sameBorder(a.border, b.border) && sameShadows(a.shadows, b.shadows) && samePaintBounds(a.paintBounds, b.paintBounds) &&
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
        fillAlpha: n.fillAlpha ?? 1,
        radius: n.radius ?? 0,
        text: n.text ?? '',
        fontSize: n.fontSize ?? 0,
        fontWeight: n.fontWeight ?? 400,
        textAlign: n.textAlign ?? 'left',
        name: n.name ?? '',
        clip: n.clip ?? false,
        visible: (n.visible ?? true) && (parent ? parent.visible : true),
        opacity: n.opacity ?? 1,
        gradient: n.gradient,
        image: n.image,
        border: n.border,
        shadows: n.shadows,
        paintBounds: n.paintBounds,
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

/** The rectangle of a node as a capture takes it: the node with the room its paint takes beyond it (the scene's `paintBounds`: the band of its border that lies outside it, not the inside, and its shadows). */
function paintedRect(n: ResolvedNode): Rect {
  const p = n.paintBounds;
  return p ? { x: n.x - p.left, y: n.y - p.top, w: n.w + p.left + p.right, h: n.h + p.top + p.bottom } : { x: n.x, y: n.y, w: n.w, h: n.h };
}

/** The world rectangle to capture for a node, with its outside border and its shadows in it; null for an unknown id. */
export function captureRectOf(rs: ResolvedScene, id: string): Rect | null {
  const n = rs.byId.get(id);
  return n ? paintedRect(n) : null;
}

/**
 * The rectangle around every visible node on the canvas as a capture takes it (their outside borders and shadows
 * included), or null for an empty scene. What a clipping ancestor cuts is not in it, as it is not on the canvas; a node
 * inside a frame that does not clip counts, so a shadow that reaches out of its page is in the picture.
 */
export function captureBounds(rs: ResolvedScene): Rect | null {
  const rects: Rect[] = [];
  for (const n of rs.items) {
    if (!n.visible) continue;
    const r = n.clipRect ? intersectRects(paintedRect(n), n.clipRect) : paintedRect(n);
    if (!n.clipRect || (r.w > 0 && r.h > 0)) rects.push(r);
  }
  return unionRects(rects);
}
