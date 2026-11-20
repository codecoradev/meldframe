// The layers panel as rows: the tree of the document, front node first, children under their parent. Pure.
import type { Node, NodeKind } from '@meldframe/protocol';
import type { SceneNode } from '@meldframe/web-canvas/scene';

export interface LayerRow {
  id: string;
  name: string;
  kind: NodeKind;
  depth: number;
  hasChildren: boolean;
  collapsed: boolean;
  selected: boolean;
  /** The actor of the last change to the node, as the server stamped it. */
  actor: string | null;
}

/** The rows to show, from the nodes and the scene that was resolved from them. A collapsed node keeps its row and hides everything below it. */
export function layerRows(
  nodes: readonly Node[],
  scene: { nodes: readonly Pick<SceneNode, 'id' | 'parent'>[] },
  collapsed: readonly string[],
  selection: readonly string[],
  lastActor: Readonly<Record<string, string>>,
): LayerRow[] {
  const byId = new Map(nodes.map((n) => [n.id, n]));
  // The scene (from the render step) has parents before children and siblings in the file's order: reuse it, then show the front first.
  const children = new Map<string | null, string[]>();
  for (const s of scene.nodes) {
    const parent = s.parent !== null && byId.has(s.parent) ? s.parent : null;
    (children.get(parent) ?? children.set(parent, []).get(parent)!).push(s.id);
  }
  const hidden = new Set(collapsed);
  const picked = new Set(selection);
  const rows: LayerRow[] = [];
  const visit = (parent: string | null, depth: number) => {
    for (const id of [...(children.get(parent) ?? [])].reverse()) {
      const n = byId.get(id)!;
      const below = children.get(id) ?? [];
      rows.push({
        id, name: n.name, kind: n.type, depth, hasChildren: below.length > 0, collapsed: hidden.has(id),
        selected: picked.has(id), actor: lastActor[id] ?? null,
      });
      if (!hidden.has(id)) visit(id, depth + 1);
    }
  };
  visit(null, 0);
  return rows;
}

/** An actor id (`agent:claude`, `person:ana`) as a kind and a name; an id without a known kind is a person. */
export function actorLabel(actor: string): { kind: 'agent' | 'person'; name: string } {
  const i = actor.indexOf(':');
  const head = i < 0 ? '' : actor.slice(0, i);
  if (head === 'agent' || head === 'person') return { kind: head, name: actor.slice(i + 1) };
  return { kind: 'person', name: actor };
}

/**
 * Which of the three agent colours an actor wears: the same one every time, from a stable hash of its id, so two
 * agents are told apart by colour as well as by name (the name is always on offer; colour is never the only signal).
 */
export function actorTone(actor: string): 1 | 2 | 3 {
  let h = 0;
  for (let i = 0; i < actor.length; i++) h = (h * 31 + actor.charCodeAt(i)) >>> 0;
  return ((h % 3) + 1) as 1 | 2 | 3;
}
