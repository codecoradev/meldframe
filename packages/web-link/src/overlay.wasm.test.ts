import { existsSync, readFileSync } from 'node:fs';
import type { Action, AppState } from '@meldframe/protocol';
import { beforeAll, describe, expect, it } from 'vitest';
import type { Engine as EngineType } from './engine';

// The open state of the shortcut sheet, the command palette, the manage view and Import variables is the app layer's (finding T11). These
// tests send the actions exactly as the pages build them, typed by the generated protocol, to the real app layer through the WASM build
// (`just wasm-build`), so a shape that serde would refuse fails here. Without the build they are skipped (the nightly wasm job runs them).
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
const state = (e: EngineType): AppState => e.snapshot().state;
const overlay = (op: 'open' | 'close' | 'toggle', which: 'help' | 'palette'): Action => ({ type: 'overlay', op: { type: op, overlay: which } });

describe.skipIf(!built)('the open state of overlays and dialogs, over the real app layer (WASM)', () => {
  it('has no overlay open at first; one is open at a time', () => {
    const e = make();
    expect(state(e).overlay).toBeUndefined();
    e.dispatch(overlay('toggle', 'help'));
    expect(state(e).overlay).toBe('help');
    e.dispatch(overlay('open', 'palette'));
    expect(state(e).overlay).toBe('palette');
    e.dispatch(overlay('close', 'help'));
    expect(state(e).overlay).toBe('palette');
    e.dispatch(overlay('close', 'palette'));
    expect(state(e).overlay).toBeUndefined();
  });

  it('keeps the palette shut while a dialog is open and says what changed in the events', () => {
    const e = make();
    e.dispatch({ type: 'openVariablesImport' });
    expect(state(e).variablesIo.importOpen).toBe(true);
    e.dispatch(overlay('toggle', 'palette'));
    expect(state(e).overlay).toBeUndefined();
    e.dispatch({ type: 'closeVariablesImport' });
    e.dispatch(overlay('toggle', 'palette'));
    expect(state(e).overlay).toBe('palette');
    expect(e.snapshot().events).toContainEqual({ type: 'overlayChanged', overlay: 'palette' });
  });

  it('opens the manage view with the search cleared, and closes it', () => {
    const e = make();
    e.dispatch({ type: 'setVariablesQuery', query: 'bg' });
    expect(state(e).variables.query).toBe('bg');
    e.dispatch({ type: 'manage', op: { type: 'open' } });
    expect(state(e).manage.open).toBe(true);
    expect(state(e).variables.query).toBe('');
    e.dispatch({ type: 'manage', op: { type: 'close' } });
    expect(state(e).manage.open).toBe(false);
  });

  it('clears the import result with the dialog open, and closes the dialog with the result', () => {
    const e = make();
    e.dispatch({ type: 'openVariablesImport' });
    e.dispatch({ type: 'importVariables', text: '' });
    expect(state(e).variablesIo.import?.status).toBe('refused');
    e.dispatch({ type: 'clearVariablesImport' });
    expect(state(e).variablesIo.import).toBeNull();
    expect(state(e).variablesIo.importOpen).toBe(true);
    e.dispatch({ type: 'importVariables', text: '' });
    e.dispatch({ type: 'closeVariablesImport' });
    expect(state(e).variablesIo.import).toBeNull();
    expect(state(e).variablesIo.importOpen).toBe(false);
  });

  it('makes the name of the chosen file safe to show before the page draws it (rule U9, issue 416), and forgets it with Clear', () => {
    const e = make();
    expect(state(e).variablesIo.importSource).toBeUndefined();
    e.dispatch({ type: 'chooseVariablesImportSource', name: 'th\u202Eeme\u200B.css' });
    expect(state(e).variablesIo.importSource).toBe('theme.css');
    e.dispatch({ type: 'chooseVariablesImportSource', name: 'a'.repeat(10_000) });
    expect(state(e).variablesIo.importSource).toHaveLength(128);
    e.dispatch({ type: 'clearVariablesImport' });
    expect(state(e).variablesIo.importSource).toBeUndefined();
  });
});
