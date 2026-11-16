import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { contrast } from './contrast';
import {
  LIGHT_DERIVED, THEMES, colorNames, colorVars, colors, hexToNumber, pixiColors, scalarVars, type ColorName, type Theme,
} from './tokens';

const css = readFileSync(new URL('./tokens.css', import.meta.url), 'utf8');
const theme = readFileSync(new URL('./app.css', import.meta.url), 'utf8');

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
  it('maps every colour into the Tailwind theme', () => {
    for (const n of colorNames) expect(theme).toContain(`--color-${n}: var(--mf-${n});`);
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
  it('marks only existing names as derived', () => {
    for (const n of LIGHT_DERIVED) expect(colorNames).toContain(n);
  });
});

/** Text pairs: 4.5:1. Non-text and large-text pairs: 3:1. */
const TEXT: [ColorName, ColorName][] = [];
for (const fg of ['text-strong', 'text', 'text-secondary', 'text-muted'] as const) {
  for (const bg of ['canvas', 'sidebar', 'panel', 'field', 'raised'] as const) TEXT.push([fg, bg]);
}
TEXT.push(
  ['chip-text', 'chip-bg'], ['on-primary', 'primary'], ['on-primary', 'danger-solid'], ['on-agent-1', 'actor-agent-1'],
  ['text', 'selection-bg'], ['success', 'panel'], ['danger', 'panel'], ['warning', 'panel'], ['info', 'panel'],
);
const NON_TEXT: [ColorName, ColorName][] = [['selection', 'canvas'], ['accent', 'panel'], ['actor-you', 'canvas']];

/** Pairs of the owner's palette that miss WCAG AA; reported to the owner, colours left as designed. */
export const KNOWN_FAILURES: string[] = [
  'dark: text-muted on field 4.46 < 4.5',
  'dark: text-muted on raised 4.46 < 4.5',
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
  it('fails only on the pairs listed in KNOWN_FAILURES', () => {
    const found = THEMES.flatMap(failures);
    if (found.length) console.log(`contrast pairs below AA:\n${found.join('\n')}`);
    expect(found).toEqual(KNOWN_FAILURES);
  });
});
