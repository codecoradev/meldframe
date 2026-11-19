import { existsSync, readFileSync } from 'node:fs';
import { beforeAll, describe, expect, it } from 'vitest';
import type { Engine as EngineType } from './engine';

// These tests drive the real app layer through the WASM build (`just wasm-build`, tools/build-wasm.sh). Without
// the build they are skipped (pull request CI builds no wasm; the nightly wasm job runs them).
const wasmFile = new URL('../../../apps/web/.wasm/meldframe_wasm_bg.wasm', import.meta.url);
const built = existsSync(wasmFile);
let Engine: typeof EngineType;

beforeAll(async () => {
  if (!built) return;
  const mod = await import('./engine');
  await mod.loadWasm(readFileSync(wasmFile));
  Engine = mod.Engine as unknown as typeof EngineType;
});

const make = () => Engine.create({ actor: 'usr_me', nodes: [], seq: 0, session: 1 });

describe.skipIf(!built)('Engine over the real app layer (WASM)', () => {
  it('starts empty with the default panels and tool', () => {
    const e = make();
    expect(e.snapshot().nodes).toEqual([]);
    expect(e.snapshot().state.tool).toBe('select');
    expect(e.snapshot().state.panels.layers).toBe(true);
  });

  it('creates a rectangle, selects it, and undo removes it again', () => {
    const e = make();
    e.dispatch({ type: 'createNode', kind: 'rectangle', x: 5, y: 6, parent: null });
    const { nodes, state } = e.snapshot();
    expect(nodes).toHaveLength(1);
    expect(state.selection).toEqual([nodes[0]!.id]);
    expect(state.properties?.x).toBe(5);
    e.dispatch({ type: 'undo' });
    expect(e.snapshot().nodes).toHaveLength(0);
    e.dispatch({ type: 'redo' });
    expect(e.snapshot().nodes).toHaveLength(1);
  });

  it('notifies subscribers with the new snapshot and the events', () => {
    const e = make();
    const seen: string[][] = [];
    e.subscribe((s) => seen.push(s.events.map((ev) => ev.type)));
    e.dispatch({ type: 'createNode', kind: 'text', x: 0, y: 0, parent: null });
    expect(seen).toHaveLength(1);
    expect(seen[0]).toContain('documentChanged');
  });

  it('hands the effects to the effect handler', () => {
    const e = make();
    const got: string[] = [];
    e.onEffects((fx) => got.push(...fx.map((f) => f.type)));
    e.dispatch({ type: 'start', nowMs: 1 });
    expect(got).toContain('subscribe');
  });

  it('turns a refused action into a refused event, not an exception', () => {
    const e = make();
    e.dispatch({ type: 'move', id: 'nope', x: 1, y: 1 });
    expect(e.snapshot().events.some((ev) => ev.type === 'refused')).toBe(true);
  });
});

describe.skipIf(!built)('Engine re-reads only what changed', () => {
  it('does not parse or notify on a tick that changes nothing', () => {
    const e = make();
    e.dispatch({ type: 'start', nowMs: 1 });
    e.dispatch({ type: 'connected', nowMs: 2 });
    const before = { ...e.stats };
    const first = e.snapshot();
    let notified = 0;
    e.subscribe(() => notified++);
    for (let i = 0; i < 5; i++) e.dispatch({ type: 'tick', nowMs: 1000 + i });
    expect(e.stats).toEqual(before);
    expect(notified).toBe(0);
    expect(e.snapshot()).toBe(first);
  });

  it('re-reads the nodes only when the document changed', () => {
    const e = make();
    e.dispatch({ type: 'createNode', kind: 'frame', x: 0, y: 0, parent: null });
    const nodes = e.snapshot().nodes;
    const reads = e.stats.nodeReads;
    e.dispatch({ type: 'setTool', tool: 'text' });
    expect(e.stats.nodeReads).toBe(reads);
    expect(e.snapshot().nodes).toBe(nodes);
    e.dispatch({ type: 'undo' });
    expect(e.stats.nodeReads).toBe(reads + 1);
  });

  it('reads the scene once per change of the document, with the nodes, and not otherwise', () => {
    const e = make();
    expect(e.stats.sceneReads).toBe(1);
    e.dispatch({ type: 'createNode', kind: 'frame', x: 3, y: 4, parent: null });
    expect(e.stats.sceneReads).toBe(2);
    const scene = e.snapshot().scene;
    expect(scene.nodes.map((n) => n.id)).toEqual(e.snapshot().nodes.map((n) => n.id));
    e.dispatch({ type: 'setTool', tool: 'text' });
    e.dispatch({ type: 'tick', nowMs: 5000 });
    expect(e.stats.sceneReads).toBe(2);
    expect(e.snapshot().scene).toBe(scene);
    e.dispatch({ type: 'undo' });
    expect(e.stats.sceneReads).toBe(3);
    expect(e.snapshot().scene.nodes).toEqual([]);
  });

  it('paints variables and a mode switch repaints without changing the document', () => {
    const nodes = [{ id: 'card', type: 'rect', pos: 'a0', fill: { var: 'bg' } }] as unknown as Parameters<typeof Engine.create>[0]['nodes'];
    const variables = {
      collections: [{ id: 't', name: 'appearance', modes: [{ id: 'light', name: 'light' }, { id: 'dark', name: 'dark' }] }],
      variables: [{ id: 'bg', collection: 't', name: 'color/bg', type: 'color', values: { light: '#ffffff', dark: '#000000' } }],
    } as Parameters<typeof Engine.create>[0]['variables'];
    const e = Engine.create({ actor: 'usr_me', nodes, seq: 0, session: 1, variables });
    expect(e.snapshot().scene.nodes[0]!.fill).toBe(0xffffff);
    const before = JSON.stringify(e.snapshot().nodes);
    const reads = { ...e.stats };
    const fx = e.dispatch({ type: 'setMode', collection: 't', mode: 'dark' });
    expect(fx).toContainEqual({ type: 'render', nodes: [] });
    expect(e.snapshot().state.modes).toEqual({ t: 'dark' });
    expect(JSON.stringify(e.snapshot().nodes)).toBe(before);
    // The scene is read again and repaints with the dark value; the nodes (the document) are not read again.
    expect(e.snapshot().scene.nodes[0]!.fill).toBe(0x000000);
    expect(e.stats.sceneReads).toBe(reads.sceneReads + 1);
    expect(e.stats.nodeReads).toBe(reads.nodeReads);
  });
});

describe.skipIf(!built)('The Variables panel and the mode over the real app layer (WASM)', () => {
  const nodes = [{ id: 'card', type: 'rect', pos: 'a0', fill: { var: 'bg' } }] as unknown as import('@meldframe/protocol').Node[];
  const variables = {
    collections: [{ id: 't', name: 'appearance', modes: [{ id: 'light', name: 'light' }, { id: 'dark', name: 'dark' }] }],
    variables: [{ id: 'bg', collection: 't', name: 'color/bg', type: 'color', values: { light: '#ffffff', dark: '#000000' } }],
  } as import('@meldframe/protocol').Variables;
  const make2 = () => Engine.create({ actor: 'usr_me', nodes, seq: 0, session: 1, variables });
  const fill = (e: EngineType) => e.snapshot().scene.nodes.find((n) => n.id === 'card')?.fill;

  it('switching the mode repaints the scene and leaves the nodes as they were', () => {
    const e = make2();
    const first = fill(e);
    const nodesBefore = e.snapshot().nodes;
    e.dispatch({ type: 'setMode', collection: 't', mode: 'dark' });
    expect(fill(e)).not.toEqual(first);
    expect(e.snapshot().nodes).toBe(nodesBefore);
    e.dispatch({ type: 'setMode', collection: 't', mode: null });
    expect(fill(e)).toEqual(first);
  });

  it('lists the variables once the panel is open and follows the search', () => {
    const e = make2();
    expect(e.snapshot().state.variables.collections).toEqual([]);
    e.dispatch({ type: 'setPanel', panel: 'variables', open: true });
    expect(e.snapshot().state.variables.collections[0]?.variables[0]?.id).toBe('bg');
    e.dispatch({ type: 'setVariablesQuery', query: 'zzz' });
    expect(e.snapshot().state.variables.matched).toBe(0);
  });
});
