import { existsSync, readFileSync } from 'node:fs';
import type { Node } from '@meldframe/protocol';
import type { MoveIntent, ResizeIntent, CreateIntent } from '@meldframe/web-canvas/interaction';
import { createActionFor, keyAction, movePreviewFor, moveActionsFor, resizeActionFor, worldOrigin } from '@meldframe/web-document/edit';
import { beforeAll, describe, expect, it } from 'vitest';
import type { Engine as EngineType } from '@meldframe/web-link/engine';

// The layout interaction of the web (a drop, a handle, a drawn box) against the real app layer through the WASM build
// (`just wasm-build`). Without the build the tests are skipped, as those of web-link are.
const wasmFile = new URL('../../../apps/web/.wasm/meldframe_wasm_bg.wasm', import.meta.url);
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

  it('a drop of the dragged nodes is the same ops, the same document and the same undo steps as the moves it was before', () => {
    // [dragged ids, selection]: one node, the selection, a node with its descendant, and ids that are not the selection.
    const cases: [string[], string[]][] = [...ids.map((id): [string[], string[]] => [[id], [id]]), [['a', 'b'], ['a', 'b']], [['a', 'k', 'g'], ['a', 'k', 'g']], [['g', 'k', 'f'], ['g', 'k', 'f']], [['a', 'b'], ['a', 'k']]];
    for (const [dragged, selection] of cases) {
      const [dx, dy] = [13, -8];
      const intent: MoveIntent = { ...gesture, ids: dragged, dx, dy, at: { x: 500, y: 40 } };
      const a = make();
      const b = make();
      a.dispatch({ type: 'select', ids: selection });
      b.dispatch({ type: 'select', ids: selection });
      const now = moveActionsFor(intent);
      expect(now).toEqual([{ type: 'dropNodes', ids: dragged, x: 500, y: 40, dx, dy }]);
      const sent = a.dispatch(now[0]!);
      // What the web sent before: one dropNode of a single top node, else the selection as one moveBy when it is the dragged
      // ids and none is under another, else a move of each top node.
      const parentOf = (id: string) => file.find((n) => n.id === id)?.parentId ?? null;
      const tops = dragged.filter((id) => !dragged.includes(parentOf(id) ?? '') && !dragged.includes(parentOf(parentOf(id) ?? '') ?? ''));
      const whole = tops.length === dragged.length && dragged.length === selection.length && dragged.every((id) => selection.includes(id));
      const before = tops.length === 1 ? [b.dispatch({ type: 'dropNode', id: tops[0]!, x: 500, y: 40, dx, dy })] : whole ? [b.dispatch({ type: 'moveBy', dx, dy })] : tops.map((id) => { const n = file.find((x) => x.id === id)!; return b.dispatch({ type: 'move', id, x: n.x! + dx, y: n.y! + dy }); });
      const ops = (fx: unknown[]) => JSON.stringify(fx.flat().flatMap((e) => ((e as { type: string; ops?: { command: unknown }[] }).type === 'sendOps' ? (e as { ops: { command: unknown }[] }).ops.map((o) => o.command) : [])));
      expect(ops(sent), `${dragged} / ${selection}`).toBe(ops(before));
      expect(JSON.stringify(a.snapshot().nodes)).toBe(JSON.stringify(b.snapshot().nodes));
      expect(a.snapshot().state.history).toEqual(b.snapshot().state.history);
    }
  });

  it('an arrow key does nothing at all: no effect, no event, the same state and nodes', () => {
    const e = make();
    e.dispatch({ type: 'select', ids: ['b'] });
    const before = JSON.stringify([e.snapshot().state, e.snapshot().nodes]);
    for (const key of ['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight']) {
      const a = keyAction({ key, metaKey: false, ctrlKey: false, shiftKey: false, altKey: false }, ['b'])!;
      expect(e.dispatch(a)).toEqual([]);
    }
    expect(JSON.stringify([e.snapshot().state, e.snapshot().nodes])).toBe(before);
  });

  it('a drag has no slot to show, so the box follows the pointer', () => {
    const e = make();
    const i: MoveIntent = { ...gesture, final: false, ids: ['a'], dx: 5, dy: 5, at: { x: 60, y: 60 } };
    expect(e.dropPreview(['a'], i.at, { dx: 5, dy: 5 })).toBeNull();
    expect(movePreviewFor(i, (ids, at, by) => e.dropPreview(ids, at, by))).toEqual({ kind: 'move', ids: ['a'], dx: 5, dy: 5 });
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
    const { slot, follow } = e.dropPreview(['t'], at, { dx: 0, dy: -50 })!;
    expect(slot).toMatchObject({ frame: 'f', index: 0 });
    expect(slot!.line.h).toBe(0);
    expect(follow).toEqual([]);
    const i: MoveIntent = { ...gesture, final: false, ids: ['t'], dx: 0, dy: -50, at };
    expect(movePreviewFor(i, (ids, p, by) => e.dropPreview(ids, p, by))).toEqual({ kind: 'slot', line: slot!.line, follow: [], dx: 0, dy: -50 });
    expect(order(e)).toEqual(['a', 'b', 't']);
    e.dispatch(moveActionsFor({ ...i, final: true })[0]!);
    expect(order(e)).toEqual(['t', 'a', 'b']);
    e.dispatch({ type: 'undo' });
    expect(order(e)).toEqual(['a', 'b', 't']);
  });

  it('an arrow key along the flow moves the child one place in one step and writes no position; the key across does nothing', () => {
    const e = make();
    e.setLayoutOnCanvas(true);
    e.dispatch({ type: 'select', ids: ['b'] });
    const key = (name: string) => keyAction({ key: name, metaKey: false, ctrlKey: false, shiftKey: false, altKey: false }, ['b'])!;
    const stored = JSON.stringify(e.snapshot().nodes.map((n) => [n.id, n.x, n.y]));
    expect(e.dispatch(key('ArrowLeft'))).toEqual([]);
    e.dispatch(key('ArrowUp'));
    expect(order(e)).toEqual(['b', 'a', 't']);
    e.dispatch(key('ArrowDown'));
    e.dispatch(key('ArrowDown'));
    expect(order(e)).toEqual(['a', 't', 'b']);
    expect(JSON.stringify(e.snapshot().nodes.map((n) => [n.id, n.x, n.y]))).toBe(stored);
    expect(e.snapshot().state.history.undoSteps).toBe(3);
    e.dispatch({ type: 'undo' });
    expect(order(e)).toEqual(['a', 'b', 't']);
  });

  it('a drag of several flow children of one frame moves them as one block to the slot, in order, writing no position, in one step', () => {
    const e = make();
    e.setLayoutOnCanvas(true);
    const { nodes, scene } = e.snapshot();
    const f = scene.nodes.find((n) => n.id === 'f')!;
    const t = scene.nodes.find((n) => n.id === 't')!;
    // The pointer is past the middle of the text, the last child: a and b go after it.
    const at = { x: f.x + 20, y: f.y + t.y + t.h + 5 };
    const i: MoveIntent = { ...gesture, final: false, ids: ['b', 'a'], dx: 0, dy: 80, at };
    const p = movePreviewFor(i, (ids, pt, by) => e.dropPreview(ids, pt, by));
    expect(p).toMatchObject({ kind: 'slot', follow: [] });
    e.dispatch({ type: 'select', ids: ['a', 'b'] });
    const actions = moveActionsFor({ ...i, final: true });
    expect(actions).toHaveLength(1);
    e.dispatch(actions[0]!);
    expect(order(e)).toEqual(['t', 'a', 'b']);
    for (const id of ['a', 'b']) {
      const was = nodes.find((n) => n.id === id)!;
      const now = e.snapshot().nodes.find((n) => n.id === id)!;
      expect([now.x, now.y, now.parentId]).toEqual([was.x, was.y, 'f']);
    }
    expect(e.snapshot().state.history.undoSteps).toBe(1); // the selection is not a step; the drop is one
    e.dispatch({ type: 'undo' });
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
