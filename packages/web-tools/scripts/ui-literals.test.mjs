import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
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

describe('ui literals: wider coverage', () => {
  it('matches units and colour functions in any case', () => {
    expect(kinds('a { width: 12PX; }')).toEqual(['1:size']);
    expect(kinds('a { transition: opacity 200MS; }')).toEqual(['1:duration']);
    expect(kinds('a { color: RGB(0 0 0); }')).toEqual(['1:colour']);
    expect(kinds('a { color: #FFF; }')).toEqual(['1:colour']);
  });
  it('flags negative sizes and durations', () => {
    expect(kinds('a { margin-left:-12px; }')).toEqual(['1:size']);
    expect(kinds('a { transition-delay: -200ms; }')).toEqual(['1:duration']);
  });
  it('flags color(), color-mix() and light-dark()', () => {
    expect(kinds('a { color: color(display-p3 1 0 0); }')).toEqual(['1:colour']);
    expect(kinds('a { color: color-mix(in srgb, var(--mf-text) 50%, var(--mf-canvas)); }')).toEqual(['1:colour']);
    expect(kinds('a { color: light-dark(var(--mf-text), var(--mf-canvas)); }')).toEqual(['1:colour']);
  });
  it('flags named colours in SVG attributes, style directives, object keys, shorthands and Tailwind values', () => {
    expect(kinds('<rect fill="red" />')).toEqual(['1:colour']);
    expect(kinds("<path stroke='blue' />")).toEqual(['1:colour']);
    expect(kinds('<p style:color="red">x</p>')).toEqual(['1:colour']);
    expect(kinds("const s = { backgroundColor: 'red' };", 'src/a.ts')).toEqual(['1:colour']);
    expect(kinds('a { border-top: thin solid red; }')).toEqual(['1:colour']);
    expect(kinds('a { background: linear-gradient(red, blue); }')).toEqual(['1:colour']);
    expect(kinds('a { box-shadow: 0 0 red; }')).toEqual(['1:colour']);
    expect(kinds('<p class="bg-[red]">x</p>')).toEqual(['1:colour']);
    expect(kinds('<p class="text-[#fff]">x</p>')).toEqual(['1:colour']);
    expect(kinds('<p class="w-[240px]">x</p>')).toEqual(['1:size']);
    expect(kinds('<p class="border-[1px_solid_tan]">x</p>')).toEqual(['1:size', '1:colour'].sort());
  });
  it('flags bare numeric durations of Svelte transitions', () => {
    expect(kinds('<div transition:fade={{ duration: 200 }}></div>')).toEqual(['1:duration']);
    expect(kinds('<div in:fly={{ y: 0, delay: 50 }}></div>')).toEqual(['1:duration']);
  });
  it('does not flag token reads, URLs in comments or words that only contain a colour name', () => {
    expect(kinds('<rect fill="var(--mf-accent)" />')).toEqual([]);
    expect(kinds('a { background: var(--mf-panel); border: 0; box-shadow: var(--mf-shadow-float); }')).toEqual([]);
    expect(kinds('a { background: url(/img/red.png); }')).toEqual([]);
    expect(kinds('/* see url(//cdn.example/red.css) 12px */ a { margin: 0; }')).toEqual([]);
    expect(kinds('// url(//cdn.example) #fff 12px', 'src/a.ts')).toEqual([]);
    expect(kinds("const s = { color: tokens.red, fill: colors.dark.accent };", 'src/a.ts')).toEqual([]);
    expect(kinds('<p class="bg-panel text-text-muted p-8 h-10 w-[var(--mf-space-16)]">x</p>')).toEqual([]);
    expect(kinds('const x = 3; // 200ms')).toEqual([]);
  });
  it('strips comments but keeps a literal after a quoted //', () => {
    expect(kinds("const u = 'http://x'; const c = '#ff0000';", 'src/a.ts')).toEqual(['1:colour']);
    expect(kinds('const u = "a // b"; const c = "#ff0000";', 'src/a.ts')).toEqual(['1:colour']);
    expect(kinds("const u = 'a'; // '#ff0000'", 'src/a.ts')).toEqual([]);
    expect(kinds('<!-- one\n  #fff 12px\n-->\n<p>x</p>')).toEqual([]);
  });
  it('exempts the tokens module by exact path', () => {
    expect(findLiterals('src/tokens.ts', "const a = '#fff'").length).toBe(1); // the lib flags it; isExempt skips it
    expect(isExempt('src/tokens.ts')).toBe(true);
    expect(isExempt('src/tokens.test.mts')).toBe(true);
    expect(isExempt('src/a.js')).toBe(false);
  });
});

describe('document data and packages', () => {
  it('exempts the sample document colours by name, not the rest of the harness', () => {
    expect(isExempt('src/harness/sample.ts')).toBe(true);
    expect(isExempt('src/harness/Harness.svelte')).toBe(false);
    expect(isExempt('src/harness/other.ts')).toBe(false);
    expect(isExempt('src/sample.ts')).toBe(false);
    expect(findLiterals('src/harness/Harness.svelte', 'const c = 0xe6ece8;').map((f) => f.kind)).toEqual(['colour']);
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
  it.each(['a.js', 'a.svg', 'a.tsx', 'a.mts', 'a.json'])('scans %s too', (file) => {
    const r = run(file, '{"x": "#ff0000"}\n');
    expect(r.status).toBe(1);
  });
  it('also scans packages/web-*/src beside the app', () => {
    const dir = mkdtempSync(join(tmpdir(), 'ui-literals-'));
    try {
      mkdirSync(join(dir, 'apps', 'web', 'src'), { recursive: true });
      mkdirSync(join(dir, 'packages', 'web-x', 'src'), { recursive: true });
      writeFileSync(join(dir, 'packages', 'web-x', 'src', 'a.ts'), "const c = '#ff0000';\n");
      const r = spawnSync('node', [new URL('./ui-literals.mjs', import.meta.url).pathname], { cwd: join(dir, 'apps', 'web'), encoding: 'utf8' });
      expect(r.status).toBe(1);
      expect(r.stderr).toContain('packages/web-x/src/a.ts:1: colour literal');
    } finally {
      rmSync(dir, { recursive: true });
    }
  });
  it('scans a packages/web-* directory that is a symlink', () => {
    const dir = mkdtempSync(join(tmpdir(), 'ui-literals-'));
    try {
      mkdirSync(join(dir, 'apps', 'web', 'src'), { recursive: true });
      mkdirSync(join(dir, 'elsewhere', 'src'), { recursive: true });
      mkdirSync(join(dir, 'packages'));
      writeFileSync(join(dir, 'elsewhere', 'src', 'a.ts'), "const c = '#ff0000';\n");
      symlinkSync(join(dir, 'elsewhere'), join(dir, 'packages', 'web-link'));
      const r = spawnSync('node', [new URL('./ui-literals.mjs', import.meta.url).pathname], { cwd: join(dir, 'apps', 'web'), encoding: 'utf8' });
      expect(r.status).toBe(1);
      expect(r.stderr).toContain('packages/web-link/src/a.ts:1: colour literal');
    } finally {
      rmSync(dir, { recursive: true });
    }
  });
  it('passes a file that reads tokens', () => {
    expect(run('A.svelte', '<p class="text-text">x</p>\n').status).toBe(0);
  });
});
