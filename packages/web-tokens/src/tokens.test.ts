import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { contrast } from './contrast';
import {
  THEMES, colorNames, colorVars, colors, effectNames, fontSize, fonts, hexToNumber, pixiColors, radius, scalarVars, type ColorName, type Theme,
} from './tokens';

const css = readFileSync(new URL('./tokens.css', import.meta.url), 'utf8');
const theme = readFileSync(new URL('./tailwind.css', import.meta.url), 'utf8');

/** Custom properties declared in the rule blocks whose selector contains `selector`. */
function declared(selector: string, exact = false): Record<string, string> {
  const out: Record<string, string> = {};
  for (const m of css.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    const sel = m[1]!.replace(/\/\*[\s\S]*?\*\//g, '').trim();
    if (exact ? sel !== selector : !sel.includes(selector)) continue;
    for (const d of m[2]!.matchAll(/(--mf-[\w-]+)\s*:\s*([^;]+);/g)) out[d[1]!] = d[2]!.trim();
  }
  return out;
}

/** Problems in the Tailwind theme mapping of app.css: a wrong, missing or unknown entry. */
function themeErrors(source: string): string[] {
  const expected: Record<string, string> = { '--spacing': 'var(--mf-space-1)' };
  for (const n of colorNames) expected[`--color-${n}`] = `var(--mf-${n})`;
  for (const n of effectNames.filter((e) => !e.startsWith('shadow'))) expected[`--color-${n}`] = `var(--mf-${n})`;
  for (const k of Object.keys(fontSize)) expected[`--text-${k}`] = `var(--mf-font-size-${k})`;
  for (const k of Object.keys(radius)) expected[`--radius-${k}`] = `var(--mf-radius-${k})`;
  for (const k of ['float', 'popover', 'dialog']) expected[`--shadow-${k}`] = `var(--mf-shadow-${k})`;
  for (const k of Object.keys(fonts)) expected[`--font-${k}`] = `var(--mf-font-family-${k})`;
  const found = new Map<string, string>();
  for (const m of source.matchAll(/^\s*(--(?:spacing|color-[\w-]+|text-[\w-]+|radius-[\w-]+|shadow-[\w-]+|font-[\w-]+)):\s*([^;]+);/gm)) {
    found.set(m[1]!, m[2]!.trim());
  }
  const errors: string[] = [];
  for (const [k, v] of Object.entries(expected)) if (found.get(k) !== v) errors.push(`${k}: expected ${v}, found ${found.get(k)}`);
  for (const k of found.keys()) if (!(k in expected) && !['--color-transparent', '--color-current'].includes(k)) errors.push(`${k}: not a token`);
  return errors;
}

const upper = (o: Record<string, string>) => Object.fromEntries(Object.entries(o).map(([k, v]) => [k, v.toUpperCase()]));

describe('tokens.css mirrors tokens.ts', () => {
  it('declares the dark colours', () => {
    expect(upper(declared("[data-theme='dark']"))).toEqual(upper(colorVars('dark')));
  });
  it('declares the light colours under the same names', () => {
    expect(upper(declared("[data-theme='light']"))).toEqual(upper(colorVars('light')));
  });
  it('declares the scalar tokens', () => {
    expect(declared(':root', true)).toEqual(scalarVars());
  });
  it('maps every token into the Tailwind theme, and nothing else', () => {
    expect(themeErrors(theme)).toEqual([]);
  });
  it('declares the @font-face families that tokens.ts names', () => {
    const families = [...css.matchAll(/@font-face\s*\{[^}]*font-family:\s*'([^']+)'/g)].map((m) => m[1]).sort();
    const named = [fonts.sans, fonts.mono].map((f) => /'([^']+)'/.exec(f)![1]).sort();
    expect(families).toEqual(named);
  });
  it('catches a typo or a missing entry in the theme mapping', () => {
    expect(themeErrors(theme.replace('var(--mf-font-size-small)', 'var(--mf-font-size-smalll)'))).not.toEqual([]);
    expect(themeErrors(theme.replace('--radius-md: var(--mf-radius-md);', ''))).not.toEqual([]);
    expect(themeErrors(theme.replace('--color-info: var(--mf-info);', '--color-info: var(--mf-danger);'))).not.toEqual([]);
    expect(themeErrors(theme.replace('--spacing: var(--mf-space-1);', '--spacing: 0.25rem;'))).not.toEqual([]);
    expect(themeErrors(theme + '\n--shadow-extra: var(--mf-shadow-float);')).not.toEqual([]);
  });
});

describe('numeric colours for the canvas', () => {
  it('converts hex to the number PixiJS takes', () => {
    expect(hexToNumber('#161618')).toBe(0x161618);
    expect(pixiColors('dark').accent).toBe(0x3b82f6);
    expect(pixiColors('light').canvas).toBe(0xe9e9ec);
  });
  it('has the same names in every theme and valid hex values', () => {
    for (const t of THEMES) {
      expect(Object.keys(colors[t]).sort()).toEqual([...colorNames].sort());
      for (const v of Object.values(colors[t])) expect(v).toMatch(/^#[0-9A-F]{6}$/);
    }
  });
});

/** Text pairs: 4.5:1. Non-text pairs: 3:1. The pairs are those of DESIGN.md section 3.2. */
const TEXT: [ColorName, ColorName][] = [];
for (const fg of ['text', 'text-muted'] as const) {
  for (const bg of ['canvas', 'sidebar', 'surface', 'field', 'surface-raised'] as const) TEXT.push([fg, bg]);
}
TEXT.push(
  ['text-strong', 'surface'], ['chip-text', 'chip-bg'], ['on-accent', 'accent-strong'], ['on-accent', 'danger-strong'], ['on-accent', 'person'],
  ['on-agent', 'agent-1'], ['on-agent', 'agent-2'], ['on-agent', 'agent-3'], ['success', 'surface'], ['warning', 'warning-bg'],
  ['danger', 'danger-bg'], ['danger', 'surface'], ['info', 'surface'], ['review-text', 'review-bg'],
);
// Non-text: the focus ring, the actor dots and rings, and the theme switch (track on the page, thumb on the track).
const NON_TEXT: [ColorName, ColorName][] = [
  ['selection', 'canvas'], ['selection', 'surface'], ['selection', 'field'], ['accent', 'surface'], ['person', 'surface'],
  ['agent-1', 'surface'], ['agent-2', 'surface'], ['agent-3', 'surface'], ['text-muted', 'canvas'], ['accent', 'canvas'],
  ['canvas', 'text-muted'], ['canvas', 'accent'],
];

function failures(t: Theme): string[] {
  const out: string[] = [];
  const check = (list: [ColorName, ColorName][], min: number) => {
    for (const [fg, bg] of list) {
      const r = contrast(colors[t][fg], colors[t][bg]);
      if (r < min) out.push(`${t}: ${fg} on ${bg} ${r.toFixed(2)} < ${min}`);
    }
  };
  check(TEXT, 4.5);
  check(NON_TEXT, 3);
  return out;
}

describe('contrast (WCAG AA)', () => {
  it('has the known ratios right', () => {
    expect(contrast('#000000', '#FFFFFF')).toBeCloseTo(21, 5);
    expect(contrast('#767676', '#FFFFFF')).toBeCloseTo(4.54, 1);
  });
  it('passes on every pair, in both themes', () => {
    expect(THEMES.flatMap(failures)).toEqual([]);
  });
});
