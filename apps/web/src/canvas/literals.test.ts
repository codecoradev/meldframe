// The canvas reads every UI colour and size from the tokens module (U4). `cargo xtask ui-literals` covers the
// styles of apps/web; this test covers the TypeScript and Svelte files of the canvas, which the Rust lint does
// not read. The sample scenes of the harness hold the colours of a sample document and are exempt.
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { describe, expect, it } from 'vitest';

const root = __dirname;
const EXEMPT = new Set(['harness/sample.ts']);

/** Colour literals on a line: hex colours, 0xRRGGBB numbers and colour functions. */
export function literalsIn(line: string): string[] {
  const code = line.replace(/\/\/.*$/, '').replace(/\/\*.*?\*\//g, '');
  return [
    ...(code.match(/#[0-9a-fA-F]{3,8}\b/g) ?? []),
    ...(code.match(/\b0x[0-9a-fA-F]{6,8}\b/g) ?? []),
    ...(code.match(/\b(?:rgb|rgba|hsl|hsla|hwb|lab|lch|oklab|oklch)\(/g) ?? []),
  ];
}

function sources(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) return sources(p);
    return /\.(ts|svelte)$/.test(name) && !name.endsWith('.test.ts') ? [p] : [];
  });
}

describe('canvas literals', () => {
  it('the scanner finds colour literals (negative check)', () => {
    expect(literalsIn("const c = '#60a5fa';")).toEqual(['#60a5fa']);
    expect(literalsIn('g.fill(0xff00aa);')).toEqual(['0xff00aa']);
    expect(literalsIn('color: rgba(0, 0, 0, 0.5)')).toEqual(['rgba(']);
    expect(literalsIn('g.fill(pal.selection); // not #fff')).toEqual([]);
  });

  it('no canvas source holds a colour literal', () => {
    const found: string[] = [];
    for (const file of sources(root)) {
      const rel = relative(root, file);
      if (EXEMPT.has(rel)) continue;
      readFileSync(file, 'utf8').split('\n').forEach((line, i) => {
        for (const lit of literalsIn(line)) found.push(`${rel}:${i + 1}: ${lit}`);
      });
    }
    expect(found).toEqual([]);
  });
});
