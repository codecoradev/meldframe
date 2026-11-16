// The layers panel as plain data: rows in display order, built from the nodes and the app state.
import type { Node } from '@meldframe/protocol';

export interface LayerRow {
  id: string;
  name: string;
  kind: Node['type'];
  depth: number;
  hasChildren: boolean;
  collapsed: boolean;
  /** The actor of the last change, as the server stamped it; absent before any event. */
  actor?: string;
}

const byPos = (a: Node, b: Node) =>
  (a.pos ?? '') < (b.pos ?? '') ? -1 : (a.pos ?? '') > (b.pos ?? '') ? 1 : a.id < b.id ? -1 : a.id > b.id ? 1 : 0;

/** The children of every parent (`null` for the roots) in z order, back to front: by position, then id. One pass. */
export function childrenOf(nodes: Node[]): Map<string | null, Node[]> {
  const map = new Map<string | null, Node[]>();
  for (const n of nodes) {
    const key = n.parentId ?? null;
    const list = map.get(key);
    if (list) list.push(n);
    else map.set(key, [n]);
  }
  for (const list of map.values()) list.sort(byPos);
  return map;
}

/** Front-most sibling first, children right under their parent, hidden children left out. */
export function layerRows(nodes: Node[], collapsed: string[], lastActor: Record<string, string>): LayerRow[] {
  const kids = childrenOf(nodes);
  const hidden = new Set(collapsed);
  const rows: LayerRow[] = [];
  const visit = (parentId: string | null, depth: number) => {
    const list = kids.get(parentId) ?? [];
    for (let i = list.length - 1; i >= 0; i--) {
      const n = list[i]!;
      const hasChildren = kids.has(n.id);
      const isCollapsed = hidden.has(n.id);
      const actor = Object.hasOwn(lastActor, n.id) ? lastActor[n.id] : undefined;
      rows.push({ id: n.id, name: n.name, kind: n.type, depth, hasChildren, collapsed: isCollapsed, actor });
      if (hasChildren && !isCollapsed) visit(n.id, depth + 1);
    }
  };
  visit(null, 0);
  return rows;
}
