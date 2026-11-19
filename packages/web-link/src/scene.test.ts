import { readFileSync } from 'node:fs';
import type { Node } from '@meldframe/protocol';
import { resolveScene } from '@meldframe/web-canvas/scene';
import { sceneFromNodes } from '@meldframe/web-document/scene';
import { describe, expect, it } from 'vitest';
import { type PaintedBox, sceneFromBoxes } from './scene';

// The same file the Rust test of meldframe-render checks `resolve` against: node lists and the boxes it gives.
const fixtures: { name: string; nodes: Node[]; boxes: PaintedBox[] }[] = JSON.parse(
  readFileSync(new URL('../../../crates/core/meldframe-render/tests/fixtures/scene.json', import.meta.url), 'utf8'),
);

describe('the Rust render step and the TypeScript scene agree (E3.1 parity)', () => {
  for (const c of fixtures) {
    it(c.name, () => {
      const fromRust = resolveScene(sceneFromBoxes(c.boxes)).scene.items;
      expect(fromRust).toHaveLength(c.nodes.length);
      expect(fromRust).toEqual(resolveScene(sceneFromNodes(c.nodes)).scene.items);
    });
  }
});
