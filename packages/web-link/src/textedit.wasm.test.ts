import { existsSync, readFileSync } from 'node:fs';
import type { Node } from '@meldframe/protocol';
import { beforeAll, describe, expect, it } from 'vitest';
import type { Engine as EngineType } from './engine';

// The text being edited, over the real app layer (WASM; skipped without the build, as engine.wasm.test.ts is): the page only
// dispatches `textEdit` and reads `state.textEdit` (decision 43, Q6). The rules are asserted in Rust; this is the bridge.
const wasmFile = new URL('../../../apps/web/.wasm/meldframe_wasm_bg.wasm', import.meta.url);
const built = existsSync(wasmFile);
let Engine: typeof EngineType;

beforeAll(async () => {
  if (!built) return;
  const mod = await import('./engine');
  await mod.loadWasm(readFileSync(wasmFile));
  Engine = mod.Engine as unknown as typeof EngineType;
});

const nodes = [
  { id: 'page', type: 'frame', pos: 'a0', x: 0, y: 0, w: 400, h: 300 },
  { id: 'box', type: 'rect', parentId: 'page', pos: 'a1', x: 0, y: 0, w: 50, h: 50 },
  { id: 'hi', type: 'text', parentId: 'page', pos: 'a2', x: 10, y: 10, w: 80, h: 20, text: 'Hello', fontSize: 16 },
] as unknown as Node[];
const make = () => Engine.create({ actor: 'usr_me', nodes, seq: 0, session: 1 });

describe.skipIf(!built)('the text being edited, over the real app layer (WASM)', () => {
  it('a double click on a text node begins an edit with its text and selects it; on a rectangle it does nothing', () => {
    const e = make();
    e.dispatch({ type: 'textEdit', op: { type: 'begin', id: 'box' } });
    expect(e.snapshot().state.textEdit).toBeUndefined();
    expect(e.snapshot().events).toEqual([]);
    e.dispatch({ type: 'textEdit', op: { type: 'begin', id: 'hi' } });
    expect(e.snapshot().state.textEdit).toEqual({ id: 'hi', text: 'Hello' });
    expect(e.snapshot().state.selection).toEqual(['hi']);
    expect(e.snapshot().events.map((x) => x.type)).toContain('textEditChanged');
  });

  it('Enter begins on exactly one selected text node and on nothing else', () => {
    const e = make();
    e.dispatch({ type: 'select', ids: ['box'] });
    e.dispatch({ type: 'textEdit', op: { type: 'beginSelected' } });
    expect(e.snapshot().state.textEdit).toBeUndefined();
    e.dispatch({ type: 'select', ids: ['hi'] });
    e.dispatch({ type: 'textEdit', op: { type: 'beginSelected' } });
    expect(e.snapshot().state.textEdit?.id).toBe('hi');
  });

  it('a changed commit is one op and one undo step; an unchanged one and a cancel make none', () => {
    const e = make();
    const sent = (fx: ReturnType<typeof e.dispatch>) => fx.filter((f) => f.type === 'sendOps');
    e.dispatch({ type: 'connected', nowMs: 0 });
    e.dispatch({ type: 'textEdit', op: { type: 'begin', id: 'hi' } });
    e.dispatch({ type: 'textEdit', op: { type: 'commit', id: 'hi', text: 'Hello world' } });
    expect(e.snapshot().state.textEdit).toBeUndefined();
    expect(e.snapshot().nodes.find((n) => n.id === 'hi')?.text).toBe('Hello world');
    expect(e.snapshot().state.history.undoSteps).toBe(1);
    e.dispatch({ type: 'textEdit', op: { type: 'begin', id: 'hi' } });
    expect(sent(e.dispatch({ type: 'textEdit', op: { type: 'commit', id: 'hi', text: 'Hello world' } }))).toEqual([]);
    e.dispatch({ type: 'textEdit', op: { type: 'begin', id: 'hi' } });
    e.dispatch({ type: 'textEdit', op: { type: 'cancel' } });
    expect(e.snapshot().state.textEdit).toBeUndefined();
    expect(e.snapshot().state.history.undoSteps).toBe(1);
    e.dispatch({ type: 'undo' });
    expect(e.snapshot().nodes.find((n) => n.id === 'hi')?.text).toBe('Hello');
  });

  it('deleting the node closes the edit', () => {
    const e = make();
    e.dispatch({ type: 'textEdit', op: { type: 'begin', id: 'hi' } });
    e.dispatch({ type: 'delete', ids: ['hi'] });
    expect(e.snapshot().state.textEdit).toBeUndefined();
  });
});
