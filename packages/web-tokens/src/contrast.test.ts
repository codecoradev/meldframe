import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { contrast, over } from './contrast';
import { THEMES, colors, effects, type ColorName, type EffectName, type Theme } from './tokens';

// The audited pairs (docs: A11y board D, DESIGN.md 3.2): the real foreground and background of text and
// interface parts, in both themes. A role is a colour name or an effect name; an effect that is translucent
// is laid over the panel (`surface`) first.
type Role = ColorName | EffectName;
function hex(t: Theme, role: Role): string {
  if (role in colors[t]) return colors[t][role as ColorName];
  return over(effects[t][role as EffectName], colors[t].surface);
}
const ratio = (t: Theme, fg: Role, bg: Role) => contrast(hex(t, fg), hex(t, bg));

const TEXT: [Role, Role][] = [
  ['on-accent', 'accent-strong'], ['on-accent', 'accent-hover'], ['on-accent', 'accent-pressed'], ['on-accent', 'danger-strong'], ['on-accent', 'danger-hover'], ['on-accent', 'danger-pressed'],
  ['text', 'control'], ['text-muted', 'control'], ['text-strong', 'control-selected'],
  ['text-secondary', 'surface'], ['text-secondary', 'surface-raised'], ['text-secondary', 'field'],
  ['text-muted', 'field'], ['text-muted', 'surface'], ['text-muted', 'canvas'],
  ['danger-text', 'danger-bg'], ['danger-text', 'surface'],
  ['selection-text', 'selection-bg'], ['text-strong', 'selection-bg'],
  ['chip-text', 'chip-bg'], ['review-text', 'review-bg'],
  // The toast and the banners: their text on their fill, and the dismiss icon and the button label on that fill.
  ['warning', 'warning-bg'], ['danger-text', 'danger-bg'], ['text-muted', 'warning-bg'], ['text-muted', 'danger-bg'],
];
const PARTS: [Role, Role][] = [
  ['icon-muted', 'surface'], ['icon-muted', 'field'], ['icon-muted', 'surface-raised'],
  ['selection', 'surface'], ['selection', 'control'], ['divider', 'surface'],
];

describe('audited contrast pairs, both themes', () => {
  for (const t of THEMES) {
    it(`${t}: text pairs reach 4.5:1`, () => {
      const bad = TEXT.map(([f, b]) => [`${f} on ${b}`, ratio(t, f, b)] as const).filter(([, r]) => !(r >= 4.5));
      expect(bad).toEqual([]);
    });
    it(`${t}: interface parts reach 3:1 except the divider, which only separates`, () => {
      const bad = PARTS.filter(([f]) => f !== 'divider')
        .map(([f, b]) => [`${f} on ${b}`, ratio(t, f, b)] as const)
        .filter(([, r]) => !(r >= 3));
      expect(bad).toEqual([]);
    });
  }
  it('the white label on the brand blue misses 4.5:1 in dark, so text buttons use accent-strong (A11y board: 3.68 against 5.17)', () => {
    expect(contrast(colors.dark['on-accent'], colors.dark.accent)).toBeLessThan(4.5);
    expect(contrast(colors.dark['on-accent'], colors.dark.accent)).toBeCloseTo(3.68, 2);
    expect(contrast(colors.dark['on-accent'], colors.dark['accent-strong'])).toBeCloseTo(5.17, 2);
  });
  it('text-faint stays decorative: it misses 4.5:1 on every panel fill in both themes (2.15 on surface in dark)', () => {
    for (const t of THEMES) for (const bg of ['surface', 'field', 'canvas'] as const) expect(ratio(t, 'text-faint', bg)).toBeLessThan(4.5);
    expect(ratio('dark', 'text-faint', 'surface')).toBeCloseTo(2.15, 2);
  });
  it('icon-muted is for the idle panel: on a hovered, selected or control fill it misses 3:1 in dark (2.96, 2.53, 2.79, 2.34), so icons there use text-muted', () => {
    for (const bg of ['hairline', 'selection-bg', 'control', 'control-selected'] as const) {
      expect(ratio('dark', 'icon-muted', bg)).toBeLessThan(3);
      expect(ratio('dark', 'text-muted', bg)).toBeGreaterThanOrEqual(3);
      expect(ratio('light', 'text-muted', bg)).toBeGreaterThanOrEqual(3);
    }
    expect(ratio('dark', 'icon-muted', 'hairline')).toBeCloseTo(2.96, 2);
    expect(ratio('dark', 'icon-muted', 'selection-bg')).toBeCloseTo(2.53, 2);
    expect(ratio('dark', 'icon-muted', 'control')).toBeCloseTo(2.79, 2);
  });
  it('text-muted on control-selected misses 4.5:1 in dark (4.41): it is for icons, not for text', () => {
    expect(ratio('dark', 'text-muted', 'control-selected')).toBeLessThan(4.5);
    expect(ratio('dark', 'text-muted', 'control-selected')).toBeCloseTo(4.41, 2);
    expect(ratio('dark', 'text-muted', 'control-selected')).toBeGreaterThan(3);
  });
});

// Where the pairs are used: a class list that puts on-accent text on the brand blue, or text-faint on real text.
const ROOTS = ['../../../apps/web/src', '../../web-canvas/src', '../../web-input/src', '../../web-panels/src', '../../web-activity/src', '../../web-feedback/src', '../../web-export/src', '../../web-document/src', '../../web-assets/src', '../../web-link/src', '../../web-chrome/src'];
function* sources(dir: string): Generator<string> {
  let entries;
  try {
    entries = readdirSync(dir, { withFileTypes: true });
  } catch {
    return;
  }
  for (const e of entries) {
    const p = join(dir, e.name);
    if (e.isDirectory()) yield* sources(p);
    else if (/\.(?:svelte|ts|css)$/.test(e.name) && !/\.test\.ts$/.test(e.name) && !/^tokens\.(?:css|ts)$/.test(e.name)) yield p;
  }
}

describe('the ratios written in DESIGN.md 3.2', () => {
  // `| name | \`fg\` on \`bg\` | 10.08 pass | 8.06 pass |`: the readable copy of the computed ratios must not drift.
  const text = readFileSync(new URL('../../../DESIGN.md', import.meta.url), 'utf8');
  const rows = [...text.matchAll(/^\| [^|\n]+ \| `([a-z0-9-]+)` on `([a-z0-9-]+)` \| ([\d.]+) (?:pass|fail) \| ([\d.]+) (?:pass|fail) \|$/gm)];
  it('finds the rows', () => {
    expect(rows.length).toBeGreaterThan(25);
  });
  it('has the computed ratio in every single-value row', () => {
    const wrong: string[] = [];
    for (const [, fg, bg, dark, light] of rows) {
      for (const [t, written] of [['dark', dark], ['light', light]] as const) {
        const got = ratio(t, fg as Role, bg as Role).toFixed(2);
        if (got !== written) wrong.push(`${fg} on ${bg} ${t}: table ${written}, computed ${got}`);
      }
    }
    expect(wrong).toEqual([]);
  });
});

describe('contrast in the markup', () => {
  // Whole files, not lines: a class list may be split over lines, and style blocks and .css files count too.
  const files: { file: string; text: string }[] = [];
  for (const root of ROOTS) {
    for (const file of sources(fileURLToPath(new URL(root, import.meta.url)))) {
      files.push({ file: file.split('/packages/').pop()!.split('/apps/').pop()!, text: readFileSync(file, 'utf8') });
    }
  }
  const at = (f: { file: string; text: string }, index: number) => `${f.file}:${f.text.slice(0, index).split('\n').length}`;
  const hits = (re: RegExp, within: (m: RegExpExecArray) => boolean = () => true) =>
    files.flatMap((f) => [...f.text.matchAll(new RegExp(re.source, re.flags.includes('g') ? re.flags : `${re.flags}g`))].filter(within).map((m) => at(f, m.index)));
  // Every way to put text-faint on text: the utility, a class: directive, the custom property in a colour or a style attribute.
  const faint = [
    /\b(?:class:)?text-text-faint\b/,
    /(?<![-\w])color\s*:\s*var\(--(?:mf|color)-text-faint\)/,
    /\bstyle[:=][^>]*(?:--mf-text-faint|--color-text-faint|text-faint)/,
  ];
  it('uses text-faint for no text (it is 2.15 to 2.22:1; use text-muted or text-secondary)', () => {
    for (const re of faint) expect(hits(re)).toEqual([]);
  });
  it('never sets text-on-accent on bg-accent (3.68:1; use bg-accent-strong, 5.17:1), in a class list over one line or several', () => {
    const lists = /class(?::[\w-]+)?=(?:"[^"]*"|\{[^}]*\})|\bclass\s*=\s*\{`[^`]*`\}/;
    expect(hits(lists, (m) => /(?<![-\w])bg-accent(?![-\w])/.test(m[0]) && /\btext-on-accent\b/.test(m[0]))).toEqual([]);
  });
  it('the scan finds what it looks for: a split class list and a style attribute are caught', () => {
    const lists = /class=(?:"[^"]*"|\{[^}]*\})/g;
    const split = 'class="flex\\n  bg-accent\\n  text-on-accent"';
    expect(split.replace(/\\n/g, '\n').match(lists)?.[0]).toBeDefined();
    expect(faint.some((re) => re.test('<p style="color: var(--mf-text-faint)">'))).toBe(true);
    expect(faint.some((re) => re.test('<i class:text-text-faint={x}>'))).toBe(true);
    expect(faint.some((re) => re.test('p { color: var(--mf-text-faint); }'))).toBe(true);
  });
});
