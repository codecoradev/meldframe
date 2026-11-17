// The document (the nodes of the app layer) as the scene the canvas draws. Pure; the canvas knows no document model.
import type { Node } from '@meldframe/protocol';
import type { Scene, SceneGradient, SceneNode } from '@meldframe/web-canvas/scene';

/** `#rrggbb`, `#rgb` or `#rrggbbaa` as a number (the alpha is dropped); anything else is no fill. */
export function parseFill(css: string): number | null {
  const m = /^#([0-9a-f]{3}|[0-9a-f]{6}|[0-9a-f]{8})$/i.exec(css);
  if (!m) return null;
  const h = m[1]!;
  return Number.parseInt(h.length === 3 ? [...h].map((c) => c + c).join('') : h.slice(0, 6), 16);
}

/** The colours the server accepts: #rgb, #rgba, #rrggbb and #rrggbbaa (a stop keeps its alpha, unlike a fill). */
const HEX_COLOR = /^#(?:[0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$/i;

/** The gradient of a node as the canvas draws it; none when the node has none or a stop is not a hex colour. */
function gradientOf(n: Node): { gradient: SceneGradient } | Record<string, never> {
  const g = n.gradient;
  if (!g || n.type === 'text') return {};
  if (g.stops.some((s) => !HEX_COLOR.test(s.color))) return {};
  return { gradient: { kind: g.kind, angle: g.angle, stops: g.stops.map((s) => ({ pos: s.pos, color: s.color })) } };
}

function toSceneNode(n: Node): SceneNode {
  const base = {
    id: n.id, parent: n.parentId, x: n.x, y: n.y, w: n.w, h: n.h, fill: parseFill(n.fill), name: n.name, clip: n.clip,
    // 1 is the default, so an opaque node (or a file from before opacity) carries none.
    ...(typeof n.opacity === 'number' && n.opacity < 1 ? { opacity: n.opacity } : {}),
    ...gradientOf(n),
  };
  switch (n.type) {
    case 'frame':
      return { ...base, kind: 'frame', radius: n.radius };
    case 'text':
      return { ...base, kind: 'text', text: n.text, fontSize: n.fontSize };
    case 'ellipse': // the canvas has no ellipse yet: the nearest drawing is a rectangle rounded to a pill or a circle
      return { ...base, kind: 'rect', radius: Math.min(n.w, n.h) / 2 };
    default: // rect, and a shader (drawn as its plain box until the canvas can run shaders)
      return { ...base, kind: 'rect', radius: n.radius };
  }
}

/**
 * Parents come before their children and siblings in the order of their position, then id (the file's own
 * order); a node whose parent is missing is kept as a root instead of being dropped.
 */
export function sceneFromNodes(nodes: readonly Node[]): Scene {
  const ids = new Set(nodes.map((n) => n.id));
  const byParent = new Map<string | null, Node[]>();
  for (const n of nodes) {
    const key = n.parentId !== null && ids.has(n.parentId) ? n.parentId : null;
    (byParent.get(key) ?? byParent.set(key, []).get(key)!).push(n);
  }
  // Byte order, as the file sorts: a position is a base-62 fractional index (digits, capitals, small letters), and a
  // locale-aware comparison would put `aa` before `aA`.
  const cmp = (a: string, b: string) => (a < b ? -1 : a > b ? 1 : 0);
  // As the file's own comparator: a node without a position keeps the order of the list (the sort is stable) and
  // comes before every node that has one; the id only breaks a tie between positions.
  const order = (a: Node, b: Node) => cmp(a.pos ?? '', b.pos ?? '') || ((a.pos ?? '') === '' ? 0 : cmp(a.id, b.id));
  const out: SceneNode[] = [];
  const seen = new Set<string>();
  const visit = (parent: string | null, as: string | null) => {
    for (const n of (byParent.get(parent) ?? []).sort(order)) {
      if (seen.has(n.id)) continue; // a cycle cannot loop
      seen.add(n.id);
      out.push({ ...toSceneNode(n), parent: as });
      visit(n.id, n.id);
    }
  };
  visit(null, null);
  // Nodes that only point at each other (the server refuses a cycle; this keeps a bad file visible, not hidden).
  for (const n of nodes) {
    if (seen.has(n.id)) continue;
    seen.add(n.id);
    out.push({ ...toSceneNode(n), parent: null });
    visit(n.id, n.id);
  }
  return { space: 'parent', nodes: out };
}
