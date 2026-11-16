// Geometry of the overlay layer: selection box, handles, size badge, touch rings. Pure. The overlay is not
// scaled by the camera, so everything here is in screen pixels (CSS px from the canvas's top-left corner) and
// only the positions come from the camera; strokes and handles keep their size at any zoom.
import { rectToScreen, type Camera } from './camera';
import { unionRects, type Point, type Rect, type Size } from './geometry';
import { duration } from '../tokens';

export type HandleId = 'nw' | 'n' | 'ne' | 'e' | 'se' | 's' | 'sw' | 'w';

/** Side of a square resize handle, in px (design: 6 px square with a 1.5 px stroke). */
export const HANDLE_SIZE = 8;
/** Extra reach around a handle for the pointer, in px. */
export const HANDLE_SLOP = 2;
/** An edge handle is only offered when the box is at least this long and this wide, so handles never crowd the body. */
export const HANDLE_MIN_SIDE = 24;
/** Gap between the box and the size badge, in px. */
export const BADGE_GAP = 10;
/** The ring stands this far outside the node and has rounded corners. */
export const RING_OFFSET = 6;
export const RING_RADIUS = 12;
/** Ring stroke width in px (design: 2 px). */
export const RING_STROKE = 2;
/** Space between the ring and its name label, in px. */
export const LABEL_GAP = 4;

const ORDER: readonly HandleId[] = ['nw', 'n', 'ne', 'e', 'se', 's', 'sw', 'w'];

/** The screen box of the selection: the union of the world rectangles through the camera; null for none. */
export function selectionBox(cam: Camera, worldRects: readonly Rect[]): Rect | null {
  const u = unionRects(worldRects);
  return u ? rectToScreen(cam, u) : null;
}

/** Centres of the eight handles on a screen box. */
export function handlePositions(b: Rect): Record<HandleId, Point> {
  const r = b.x + b.w;
  const bt = b.y + b.h;
  const mx = b.x + b.w / 2;
  const my = b.y + b.h / 2;
  return {
    nw: { x: b.x, y: b.y }, n: { x: mx, y: b.y }, ne: { x: r, y: b.y },
    e: { x: r, y: my }, se: { x: r, y: bt }, s: { x: mx, y: bt },
    sw: { x: b.x, y: bt }, w: { x: b.x, y: my },
  };
}

/** The handles to draw and to hit: corners always, an edge only when it is long enough. */
export function visibleHandles(b: Rect): HandleId[] {
  return ORDER.filter((h) => {
    if (h === 'n' || h === 's' || h === 'e' || h === 'w') return b.w >= HANDLE_MIN_SIDE && b.h >= HANDLE_MIN_SIDE;
    return true;
  });
}

/** The handle under the screen point, or null. The nearest centre wins; corners come first on a tie. */
export function handleAt(b: Rect, px: number, py: number): HandleId | null {
  const reach = HANDLE_SIZE / 2 + HANDLE_SLOP;
  const pos = handlePositions(b);
  let best: HandleId | null = null;
  let bestD = Infinity;
  for (const h of visibleHandles(b)) {
    const d = Math.max(Math.abs(px - pos[h].x), Math.abs(py - pos[h].y));
    if (d <= reach && d < bestD) {
      best = h;
      bestD = d;
    }
  }
  return best;
}

const CURSORS: Record<HandleId, string> = {
  nw: 'nwse-resize', se: 'nwse-resize', ne: 'nesw-resize', sw: 'nesw-resize',
  n: 'ns-resize', s: 'ns-resize', e: 'ew-resize', w: 'ew-resize',
};
export const resizeCursor = (h: HandleId): string => CURSORS[h];

/** The badge text: width times height in whole world units (not pixels, so it does not change with zoom). */
export const badgeText = (w: number, h: number): string => `${Math.round(w)} × ${Math.round(h)}`;

/** Top-left of the badge: centred under the box, moved above it when it would leave the viewport at the bottom. */
export function badgeOrigin(box: Rect, badge: Size, viewport: Size): Point {
  const x = Math.max(0, Math.min(viewport.w - badge.w, box.x + box.w / 2 - badge.w / 2));
  const below = box.y + box.h + BADGE_GAP;
  const y = below + badge.h > viewport.h ? box.y - BADGE_GAP - badge.h : below;
  return { x, y };
}

/** Opacity of a touch ring `ageMs` after the touch: a linear fade, or full for the hold time with reduced motion. */
export function touchAlpha(ageMs: number, reducedMotion: boolean): number {
  if (ageMs < 0) return 1;
  if (reducedMotion) return ageMs < duration['reduced-motion-hold'] ? 1 : 0;
  const fade = duration['touch-ring'];
  return ageMs >= fade ? 0 : 1 - ageMs / fade;
}

export interface TouchRing {
  rect: Rect;
  radius: number;
  stroke: number;
  /** Bottom-left anchor of the name label: its box sits above this point. */
  label: Point;
}

/** The ring around a screen box and where its label goes. */
export function touchRing(box: Rect): TouchRing {
  const rect = { x: box.x - RING_OFFSET, y: box.y - RING_OFFSET, w: box.w + 2 * RING_OFFSET, h: box.h + 2 * RING_OFFSET };
  return { rect, radius: RING_RADIUS, stroke: RING_STROKE, label: { x: rect.x, y: rect.y - LABEL_GAP } };
}
