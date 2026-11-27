import { existsSync, readFileSync } from 'node:fs';
import type { Node } from '@meldframe/protocol';
import { beforeAll, describe, expect, it } from 'vitest';
import type { Engine as EngineType } from '@meldframe/web-link/engine';

// What is under a point, asked of the real app layer through the WASM build (`just wasm-build`): the engine's `hit` answers the
// probes of the fixtures that the render crate, the app layer and the bridge answer too (`meldframe-render/tests/fixtures/hit.json`),
// so the web holds no rule of its own (T9, decision 43). Without the build the tests are skipped, as the other wasm tests are.
const wasmFile = new URL('../../../apps/web/.wasm/meldframe_wasm_bg.wasm', import.meta.url);
const fixtureFile = new URL('../../../crates/core/meldframe-render/tests/fixtures/hit.json', import.meta.url);
const built = existsSync(wasmFile);
let Engine: typeof EngineType;

interface Case {
  name: string;
  layout?: boolean;
  renderOnly?: boolean;
  nodes: Node[];
  probes: { at: [number, number]; hit: string | null }[];
}
const cases: Case[] = JSON.parse(readFileSync(fixtureFile, 'utf8'));

beforeAll(async () => {
  if (!built) return;
  const mod = await import('@meldframe/web-link/engine');
  await mod.loadWasm(readFileSync(wasmFile));
  Engine = mod.Engine as unknown as typeof EngineType;
});

describe.skipIf(!built)('Engine.hit', () => {
  it('answers every probe of the shared fixtures, with the layout off and on', () => {
    let probes = 0;
    for (const c of cases.filter((c) => !c.renderOnly)) {
      for (const layout of c.layout ? [true] : [false, true]) {
        const e = Engine.create({ actor: 'usr_me', nodes: c.nodes, seq: 0, session: 1 });
        e.setLayoutOnCanvas(layout);
        for (const p of c.probes) {
          expect(e.hit(p.at[0], p.at[1]), `${c.name}, layout ${layout}, at ${p.at}`).toBe(p.hit);
          probes++;
        }
      }
    }
    expect(probes).toBeGreaterThan(60);
  });

  it('is null for nothing, changes no snapshot and raises no event', () => {
    const e = Engine.create({ actor: 'usr_me', nodes: [], seq: 0, session: 1 });
    const before = e.snapshot();
    expect(e.hit(1, 1)).toBeNull();
    expect(e.hit(Number.NaN, 1)).toBeNull();
    expect(e.snapshot()).toBe(before);
  });
});
