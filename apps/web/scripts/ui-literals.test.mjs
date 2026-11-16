import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { findLiterals, isExempt } from './ui-literals-lib.mjs';

const kinds = (text, file = 'src/A.svelte') => findLiterals(file, text).map((f) => `${f.line}:${f.kind}`);

describe('ui literals', () => {
  it('flags colours in every notation', () => {
    expect(kinds('a { color: #fff; }')).toEqual(['1:colour']);
    expect(kinds('a { color: #1A2b3C80; }')).toEqual(['1:colour']);
    expect(kinds('a { color: rgba(0, 0, 0, .5); }')).toEqual(['1:colour']);
    expect(kinds('a { color: oklch(0.7 0.1 200); }')).toEqual(['1:colour']);
    expect(kinds('const c = 0xff8800;', 'src/a.ts')).toEqual(['1:colour']);
    expect(kinds("g.fill('#336699')", 'src/a.ts')).toEqual(['1:colour']);
  });
  it('flags font sizes, families, durations and pixel sizes', () => {
    expect(kinds('a { font-size: 14px; }')).toContain('1:font');
    expect(kinds('a { font-family: Inter; }')).toEqual(['1:font']);
    expect(kinds('a { transition: opacity 200ms; }')).toEqual(['1:duration']);
    expect(kinds('a { animation-duration: .3s; }')).toEqual(['1:duration']);
    expect(kinds('a { width: 240px; }')).toEqual(['1:size']);
  });
  it('passes token reads, zero, ids and comments', () => {
    expect(kinds('a { color: var(--colour-text); width: var(--size-panel-width); margin: 0; }')).toEqual([]);
    expect(kinds('a { font-family: var(--font-ui); font-size: var(--font-size-md); }')).toEqual([]);
    expect(kinds('<a href="#top" id="x#1">issue #12</a>')).toEqual([]);
    expect(kinds('// color: #fff 12px')).toEqual([]);
    expect(kinds('/* 14px #fff */ a { margin: 0; }')).toEqual([]);
    expect(kinds('const n = items.length * 2; // 300ms')).toEqual([]);
  });
  it('reports the line of each finding', () => {
    expect(kinds('a {\n  color: #fff;\n  width: 10px;\n}')).toEqual(['2:colour', '3:size']);
  });
  it('exempts the tokens module and tests only, by exact path', () => {
    expect(isExempt('src/tokens.ts')).toBe(true);
    expect(isExempt('src/tokens.css')).toBe(true);
    expect(isExempt('src/engine.test.ts')).toBe(true);
    expect(isExempt('src/panels/Layers.svelte')).toBe(false);
    expect(isExempt('src/panels/tokens.ts')).toBe(false);
    expect(isExempt('src/tokens.evil.svelte')).toBe(false);
    expect(isExempt('e2e/run.cjs')).toBe(false);
  });
  it('flags named colours, rem and em units, the font shorthand and short 0x colours', () => {
    expect(kinds('a { color: red; }')).toEqual(['1:colour']);
    expect(kinds('a { background: white; }')).toEqual(['1:colour']);
    expect(kinds('a { border-color: RebeccaPurple; }')).toEqual(['1:colour']);
    expect(kinds('a { fill: currentColor; stroke: transparent; }')).toEqual([]);
    expect(kinds('a { margin: 1.5rem; }')).toEqual(['1:size']);
    expect(kinds('a { padding: 2em; }')).toEqual(['1:size']);
    expect(kinds('a { height: 50vh; }')).toEqual(['1:size']);
    expect(kinds('a { font: 12px sans-serif; }')).toEqual(['1:font']);
    expect(kinds('a { font: var(--font-ui); font: inherit; }')).toEqual([]);
    expect(kinds('const c = 0xfff;', 'src/a.ts')).toEqual(['1:colour']);
    expect(kinds('const c = 0xff8800aa;', 'src/a.ts')).toEqual(['1:colour']);
  });
});

describe('ui-literals command', () => {
  const run = (file, body) => {
    const dir = mkdtempSync(join(tmpdir(), 'ui-literals-'));
    try {
      mkdirSync(join(dir, 'src'));
      writeFileSync(join(dir, 'src', file), body);
      return spawnSync('node', [new URL('./ui-literals.mjs', import.meta.url).pathname], { cwd: dir, encoding: 'utf8' });
    } finally {
      rmSync(dir, { recursive: true });
    }
  };
  it('fails on a literal colour in a .svelte file', () => {
    const r = run('A.svelte', '<p style="color: #ff0000">x</p>\n');
    expect(r.status).toBe(1);
    expect(r.stderr).toContain('src/A.svelte:1: colour literal');
  });
  it('passes a file that reads tokens', () => {
    expect(run('A.svelte', '<p class="text-text">x</p>\n').status).toBe(0);
  });
});
