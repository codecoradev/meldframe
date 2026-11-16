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

/** Siblings in z order, back to front: by position, then id. */
export function siblings(nodes: Node[], parentId: string | null): Node[] {
  return nodes
    .filter((n) => (n.parentId ?? null) === parentId)
    .sort((a, b) => ((a.pos ?? '') < (b.pos ?? '') ? -1 : (a.pos ?? '') > (b.pos ?? '') ? 1 : a.id < b.id ? -1 : 1));
}

/** Front-most sibling first, children right under their parent, hidden children left out. */
export function layerRows(nodes: Node[], collapsed: string[], lastActor: Record<string, string>): LayerRow[] {
  const rows: LayerRow[] = [];
  const visit = (parentId: string | null, depth: number) => {
    for (const n of siblings(nodes, parentId).reverse()) {
      const hasChildren = nodes.some((c) => c.parentId === n.id);
      const isCollapsed = collapsed.includes(n.id);
      rows.push({ id: n.id, name: n.name, kind: n.type, depth, hasChildren, collapsed: isCollapsed, actor: lastActor[n.id] });
      if (hasChildren && !isCollapsed) visit(n.id, depth + 1);
    }
  };
  visit(null, 0);
  return rows;
}
