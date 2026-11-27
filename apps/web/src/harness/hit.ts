// What the harness answers when the canvas asks what is under a point. The harness holds a `Scene` and no app layer, but the
// canvas holds no hit rule (T9, decision 43): so the question goes to the real one. This builds an `Engine` (the wasm app layer) from
// the scene the first time it is asked about a scene, and again when the scene is another one (a drag made it so), and asks that.
// It needs the wasm build (`just wasm-build`), as the editor does. Development only, like the rest of the harness.
import type { Node } from '@meldframe/protocol';
import { Engine } from '@meldframe/web-link/engine';
import type { Scene } from '@meldframe/web-canvas/scene';

/** The nodes of a scene in the document's own form: the scene's array order is paint order, which a position key says. */
export function nodesOf(scene: Scene): Node[] {
  if (scene.space !== 'parent') throw new Error('the harness hit test reads scenes in parent space');
  return scene.nodes.map((n, i) => ({
    id: n.id,
    type: n.kind,
    parentId: n.parent,
    pos: `a${String(i).padStart(7, '0')}`,
    x: n.x,
    y: n.y,
    w: n.w,
    h: n.h,
    clip: n.clip ?? false,
    text: n.text ?? '',
    fontSize: n.fontSize ?? 14,
  })) as unknown as Node[];
}

/** `hitOf()(scene)` is the function that answers what is under a world point in that scene. */
export function hitOf(): (scene: Scene) => (x: number, y: number) => string | null {
  let built: { scene: Scene; engine: Engine } | null = null;
  return (scene) => (x, y) => {
    if (built?.scene !== scene) built = { scene, engine: Engine.create({ actor: 'usr_harness', session: 1, nodes: nodesOf(scene), seq: 0 }) };
    return built.engine.hit(x, y);
  };
}
