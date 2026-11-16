// The canvas as plain data: what to draw and what a point means. No Pixi here, so it is testable.
import type { Node } from '@meldframe/protocol';
import { childrenOf } from '../layers';

export interface SceneItem {
  id: string;
  kind: Node['type'];
  /** Absolute position and size on the canvas. */
  x: number;
  y: number;
  w: number;
  h: number;
  fill: string;
  radius: number;
  text: string;
  fontSize: number;
}

/** Every node with its absolute position, back to front (a parent before its children). */
export function sceneItems(nodes: Node[]): SceneItem[] {
  const out: SceneItem[] = [];
  const kids = childrenOf(nodes);
  const visit = (parentId: string | null, ox: number, oy: number) => {
    for (const n of kids.get(parentId) ?? []) {
      const x = ox + n.x;
      const y = oy + n.y;
      out.push({
        id: n.id, kind: n.type, x, y, w: n.w, h: n.h, fill: n.fill, radius: n.radius,
        text: n.text ?? '', fontSize: n.fontSize,
      });
      visit(n.id, x, y);
    }
  };
  visit(null, 0, 0);
  return out;
}

const inside = (i: SceneItem, x: number, y: number) => x >= i.x && x <= i.x + i.w && y >= i.y && y <= i.y + i.h;

/** The front-most node under the point, or null. */
export function hitTest(items: SceneItem[], x: number, y: number): string | null {
  for (let k = items.length - 1; k >= 0; k--) if (inside(items[k]!, x, y)) return items[k]!.id;
  return null;
}

/** The frame a node made at this point belongs to, with the point in that frame's space. */
export function parentAt(items: SceneItem[], x: number, y: number): { parent: string | null; x: number; y: number } {
  for (let k = items.length - 1; k >= 0; k--) {
    const i = items[k]!;
    if (i.kind === 'frame' && inside(i, x, y)) return { parent: i.id, x: x - i.x, y: y - i.y };
  }
  return { parent: null, x, y };
}
