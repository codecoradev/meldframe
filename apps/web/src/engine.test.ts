import { readFileSync } from 'node:fs';
import { beforeAll, describe, expect, it } from 'vitest';
import { Engine, loadWasm } from './engine';

beforeAll(async () => {
  await loadWasm(readFileSync(new URL('./wasm/meldframe_wasm_bg.wasm', import.meta.url)));
});

const make = () => Engine.create({ actor: 'usr_me', nodes: [], seq: 0, session: 1 });

describe('Engine over the real app layer (WASM)', () => {
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
