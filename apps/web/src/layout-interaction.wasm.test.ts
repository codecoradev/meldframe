import { existsSync, readFileSync } from 'node:fs';
import type { Node } from '@meldframe/protocol';
import type { MoveIntent, ResizeIntent, CreateIntent } from '@meldframe/web-canvas/interaction';
import { createActionFor, movePreviewFor, moveActionsFor, resizeActionFor, worldOrigin } from '@meldframe/web-document/edit';
import { beforeAll, describe, expect, it } from 'vitest';
import type { Engine as EngineType } from '@meldframe/web-link/engine';

// The layout interaction of the web (a drop, a handle, a drawn box) against the real app layer through the WASM build
// (`just wasm-build`). Without the build the tests are skipped, as those of web-link are.
const wasmFile = new URL('../.wasm/meldframe_wasm_bg.wasm', import.meta.url);
const built = existsSync(wasmFile);
let Engine: typeof EngineType;

beforeAll(async () => {
  if (!built) return;
  const mod = await import('@meldframe/web-link/engine');
  await mod.loadWasm(readFileSync(wasmFile));
  Engine = mod.Engine as unknown as typeof EngineType;
});

const file: Node[] = [
  { id: 'f', type: 'frame', pos: 'a0', x: 40, y: 30, w: 200, h: 300, layout: 'column', padding: 10, gap: 5 },
  { id: 'a', type: 'rect', parentId: 'f', pos: 'a0', x: 7, y: 9, w: 50, h: 20 },
  { id: 'b', type: 'rect', parentId: 'f', pos: 'a1', x: 3, y: 4, w: 1, h: 30, sw: 'fill' },
  { id: 't', type: 'text', parentId: 'f', pos: 'a2', x: 0, y: 0, w: 9, h: 7, text: 'Hello', fontSize: 16, sw: 'hug', sh: 'hug' },
  { id: 'g', type: 'frame', pos: 'a1', x: 400, y: 30, w: 200, h: 200 },
  { id: 'k', type: 'rect', parentId: 'g', pos: 'a0', x: 20, y: 20, w: 30, h: 30 },
] as unknown as Node[];
const make = () => Engine.create({ actor: 'usr_me', nodes: file, seq: 0, session: 1 });
/** The children of the frame `f` in the order of the flow. */
const order = (e: EngineType) => e.snapshot().nodes.filter((n) => n.parentId === 'f').sort((x, y) => ((x.pos ?? '') < (y.pos ?? '') ? -1 : 1)).map((n) => n.id);
const gesture = { final: true, cancelled: false, mods: { shift: false, alt: false, ctrl: false, meta: false } };
const ids = file.map((n) => n.id);
const handles = ['nw', 'n', 'ne', 'e', 'se', 's', 'sw', 'w'] as const;

describe.skipIf(!built)('with the layout off the canvas', () => {
  it('draws every node at its stored box, so every origin, resize and created node is what it always was', () => {
    const e = make();
    const { nodes, scene } = e.snapshot();
    for (const n of nodes) expect(scene.nodes.find((s) => s.id === n.id)).toMatchObject({ x: n.x, y: n.y, w: n.w, h: n.h });
    for (const id of [null, ...ids]) expect(worldOrigin(nodes, id, scene)).toEqual(worldOrigin(nodes, id));
    for (const id of ids) {
      for (const handle of handles) {
        const i: ResizeIntent = { ...gesture, ids: [id], handle, dx: 9, dy: -4 };
        expect(JSON.stringify(resizeActionFor(i, nodes, scene))).toBe(JSON.stringify(resizeActionFor(i, nodes)));
      }
      const c: CreateIntent = { ...gesture, tool: 'rect', at: id, x: 60, y: 70, w: 0, h: 0 };
      expect(JSON.stringify(createActionFor(c, nodes, scene))).toBe(JSON.stringify(createActionFor(c, nodes)));
    }
  });

  it('a drop of one node is the same op, the same document and the same undo step as the move it was before', () => {
    for (const id of ids) {
      const [dx, dy] = [13, -8]; // the old action of a single selected node was moveBy
      const intent: MoveIntent = { ...gesture, ids: [id], dx, dy, at: { x: 500, y: 40 } };
      const a = make();
      const b = make();
      a.dispatch({ type: 'select', ids: [id] });
      b.dispatch({ type: 'select', ids: [id] });
      const now = moveActionsFor(intent, [id], a.snapshot().nodes);
      expect(now).toEqual([{ type: 'dropNode', id, x: 500, y: 40, dx, dy }]);
      const sent = JSON.stringify(a.dispatch(now[0]!));
      const was = JSON.stringify(b.dispatch({ type: 'moveBy', dx, dy }));
      expect(sent).toBe(was);
      expect(JSON.stringify(a.snapshot().nodes)).toBe(JSON.stringify(b.snapshot().nodes));
      expect(a.snapshot().state.history).toEqual(b.snapshot().state.history);
    }
  });

  it('a drag has no slot to show, so the box follows the pointer', () => {
    const e = make();
    const i: MoveIntent = { ...gesture, final: false, ids: ['a'], dx: 5, dy: 5, at: { x: 60, y: 60 } };
    expect(e.dropPreview('a', i.at, { dx: 5, dy: 5 })).toBeNull();
    expect(movePreviewFor(i, e.snapshot().nodes, (id, at, by) => e.dropPreview(id, at, by))).toEqual({ kind: 'move', ids: ['a'], dx: 5, dy: 5 });
  });
});

describe.skipIf(!built)('with the layout on the canvas', () => {
  it('asks the app layer where in the flow a drag would land, and the drop puts the node there', () => {
    const e = make();
    e.setLayoutOnCanvas(true);
    const { scene } = e.snapshot();
    const f = scene.nodes.find((n) => n.id === 'f')!;
    // The pointer is above the first child, inside the frame (world: the frame is at 40, 30).
    const at = { x: f.x + 20, y: f.y + 2 };
    const slot = e.dropPreview('t', at, { dx: 0, dy: -50 });
    expect(slot).toMatchObject({ frame: 'f', index: 0 });
    expect(slot!.line.h).toBe(0);
    const i: MoveIntent = { ...gesture, final: false, ids: ['t'], dx: 0, dy: -50, at };
    expect(movePreviewFor(i, e.snapshot().nodes, (id, p, by) => e.dropPreview(id, p, by))).toEqual({ kind: 'slot', line: slot!.line });
    expect(order(e)).toEqual(['a', 'b', 't']);
    e.dispatch(moveActionsFor({ ...i, final: true }, ['t'], e.snapshot().nodes)[0]!);
    expect(order(e)).toEqual(['t', 'a', 'b']);
    e.dispatch({ type: 'undo' });
    expect(order(e)).toEqual(['a', 'b', 't']);
  });

  it('a drag of several nodes is still a moveBy, documented and not guarded: the stored positions move by the drag and the flow is left alone', () => {
    const e = make();
    e.setLayoutOnCanvas(true);
    const { nodes, scene } = e.snapshot();
    const i: MoveIntent = { ...gesture, ids: ['a', 'b'], dx: 12, dy: 7, at: { x: 300, y: 300 } };
    const actions = moveActionsFor(i, ['a', 'b'], nodes);
    expect(actions).toEqual([{ type: 'moveBy', dx: 12, dy: 7 }]);
    e.dispatch({ type: 'select', ids: ['a', 'b'] });
    e.dispatch(actions[0]!);
    const after = e.snapshot();
    for (const id of ['a', 'b']) {
      const was = nodes.find((n) => n.id === id)!;
      const now = after.nodes.find((n) => n.id === id)!;
      expect([now.x, now.y, now.parentId]).toEqual([was.x + 12, was.y + 7, 'f']);
      expect(after.scene.nodes.find((n) => n.id === id)).toMatchObject(scene.nodes.find((n) => n.id === id)!); // drawn where the flow puts it
    }
    expect(order(e)).toEqual(['a', 'b', 't']);
  });

  it('a resize from the drawn box leaves the side the handle does not hold as it is: a hug text stays hug on the other axis', () => {
    const e = make();
    e.setLayoutOnCanvas(true);
    const { nodes, scene } = e.snapshot();
    const drawn = scene.nodes.find((n) => n.id === 't')!;
    const i: ResizeIntent = { ...gesture, ids: ['t'], handle: 'e', dx: 30, dy: 12 };
    const a = resizeActionFor(i, nodes, scene);
    expect(a).toEqual({ type: 'resize', id: 't', w: drawn.w + 30, h: drawn.h });
    e.dispatch(a!);
    const t = e.snapshot().nodes.find((n) => n.id === 't')!;
    expect([t.sw, t.sh]).toEqual(['fixed', 'hug']);
    // The stored box (height 7) would have been sent for the side the handle does not hold, and made that side fixed.
    expect(drawn.h).not.toBe(7);
  });
});
