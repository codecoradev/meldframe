// The tokens module of the web client: the single source of every colour, type size, spacing step,
// radius, shadow and size the web client uses (U4). The values are those of `DESIGN.md` at the
// repository root; `design-file.test.ts` fails when this file or `tokens.css` differs from its
// token block. `tokens.css` mirrors this file and `tokens.test.ts` fails when the two drift. The
// PixiJS canvas reads the numeric colours from `pixiColors`.

export const THEMES = ['dark', 'light'] as const;
export type Theme = (typeof THEMES)[number];

/** Colour roles that are plain `#RRGGBB`, so the canvas can read them as numbers. */
const dark = {
  canvas: '#161618',
  sidebar: '#1B1B1E',
  surface: '#1E1E21',
  'surface-raised': '#26262B',
  field: '#26262A',
  border: '#2E2E33',
  hairline: '#2A2A2E',
  text: '#E4E4E7',
  'text-strong': '#F4F4F5',
  'text-muted': '#A1A1AA',
  'text-faint': '#52525B',
  accent: '#3B82F6',
  'accent-strong': '#2563EB',
  'on-accent': '#FFFFFF',
  selection: '#60A5FA',
  'chip-bg': '#1F2540',
  'chip-border': '#343D6E',
  'chip-text': '#C7D2FE',
  person: '#6E60EA',
  'agent-1': '#FB923C',
  'agent-2': '#2DD4BF',
  'agent-3': '#C084FC',
  'on-agent': '#1C1917',
  success: '#4ADE80',
  warning: '#FDE68A',
  'warning-bg': '#26221A',
  danger: '#F87171',
  'danger-bg': '#2A1F1F',
  'danger-strong': '#B91C1C',
  info: '#93C5FD',
  'review-bg': '#2B2118',
  'review-text': '#FDBA74',
} as const;

export type ColorName = keyof typeof dark;

const light: Record<ColorName, string> = {
  canvas: '#E9E9EC',
  sidebar: '#F4F4F5',
  surface: '#FFFFFF',
  'surface-raised': '#FFFFFF',
  field: '#F4F4F5',
  border: '#E4E4E7',
  hairline: '#ECECEF',
  text: '#18181B',
  'text-strong': '#09090B',
  'text-muted': '#52525B',
  'text-faint': '#A1A1AA',
  accent: '#2563EB',
  'accent-strong': '#2563EB',
  'on-accent': '#FFFFFF',
  selection: '#2563EB',
  'chip-bg': '#EEF2FF',
  'chip-border': '#C7D2FE',
  'chip-text': '#3730A3',
  person: '#5B4BD6',
  'agent-1': '#C2410C',
  'agent-2': '#0E7490',
  'agent-3': '#7E22CE',
  'on-agent': '#FFFFFF',
  success: '#15803D',
  warning: '#92400E',
  'warning-bg': '#FFFBEB',
  danger: '#B91C1C',
  'danger-bg': '#FEF2F2',
  'danger-strong': '#B91C1C',
  info: '#1D4ED8',
  'review-bg': '#FFF7ED',
  'review-text': '#9A3412',
};

/** Roles that are not plain hex: translucent fills and shadows. */
const darkEffects = {
  'diff-add': 'rgba(74,222,128,0.10)',
  'diff-delete': 'rgba(248,113,113,0.10)',
  scrim: 'rgba(0,0,0,0.5)',
  'shadow-float': '0 8px 24px rgba(0,0,0,0.35)',
  'shadow-popover': '0 12px 32px rgba(0,0,0,0.5)',
} as const;

export type EffectName = keyof typeof darkEffects;

const lightEffects: Record<EffectName, string> = {
  'diff-add': 'rgba(21,128,61,0.10)',
  'diff-delete': 'rgba(185,28,28,0.10)',
  scrim: 'rgba(0,0,0,0.35)',
  'shadow-float': '0 8px 24px rgba(0,0,0,0.12)',
  'shadow-popover': '0 12px 32px rgba(0,0,0,0.18)',
};

export const colors: Record<Theme, Record<ColorName, string>> = { dark, light };
export const effects: Record<Theme, Record<EffectName, string>> = { dark: darkEffects, light: lightEffects };
export const colorNames = Object.keys(dark) as ColorName[];
export const effectNames = Object.keys(darkEffects) as EffectName[];

/** `#RRGGBB` as the number PixiJS takes. */
export const hexToNumber = (hex: string): number => Number.parseInt(hex.slice(1), 16);

/** Numeric colours of one theme, for the canvas. */
export function pixiColors(theme: Theme): Record<ColorName, number> {
  return Object.fromEntries(colorNames.map((n) => [n, hexToNumber(colors[theme][n])])) as Record<ColorName, number>;
}

/** The font stacks of `DESIGN.md`. */
export const fontsDesign = {
  sans: 'Inter, system-ui, sans-serif',
  mono: 'JetBrains Mono, ui-monospace, monospace',
} as const;

/** The stacks the page uses: the self-hosted variable fonts first, then the design stacks. */
export const fonts = {
  sans: `'Inter Variable', ${fontsDesign.sans}`,
  mono: `'JetBrains Mono Variable', ${fontsDesign.mono}`,
} as const;

/** Font sizes in px. */
export const fontSize = { caption: 11, small: 12, body: 13, title: 16, heading: 22, display: 24 } as const;
export const fontWeight = { regular: 400, medium: 500, semibold: 600, bold: 700 } as const;
export const lineHeight = { tight: 1.2, body: 1.45, relaxed: 1.5 } as const;
/** Space steps in px, keyed 1 to 6. */
export const space = { 1: 4, 2: 8, 3: 12, 4: 16, 5: 24, 6: 32 } as const;
export const radius = { xs: 4, sm: 6, md: 8, lg: 12, pill: 999 } as const;
/** Control heights and panel widths in px. */
export const size = {
  'control-sm': 24, 'control-md': 28, 'control-lg': 32, 'control-xl': 36,
  sidebar: 248, layers: 260, inspector: 288, chat: 352, files: 280,
} as const;
export const stroke = { selection: 1.5, focus: 2, 'focus-offset': 2 } as const;
/** Durations in ms. */
export const duration = { fast: 120, base: 200, slow: 320, 'touch-ring': 1500, 'reduced-motion-hold': 2000 } as const;
export const easing = { standard: 'cubic-bezier(0.2, 0, 0, 1)' } as const;

export const cssVar = (name: string): string => `--mf-${name}`;

/** Unit a plain number of `DESIGN.md` carries in CSS. */
const UNITS: Record<string, string> = {
  'font-size': 'px', space: 'px', radius: 'px', size: 'px', stroke: 'px', duration: 'ms',
};

const groups: Record<string, Record<string, string | number>> = {
  'font-size': fontSize, 'font-weight': fontWeight, 'line-height': lineHeight, space, radius, size, stroke, duration, easing,
};

/** Every token that does not depend on the theme as `[DESIGN.md key, plain value]`. */
export function scalarTokens(): [string, string][] {
  const out: [string, string][] = [
    ['font-family.sans', fontsDesign.sans],
    ['font-family.mono', fontsDesign.mono],
  ];
  for (const [group, values] of Object.entries(groups)) {
    for (const [name, value] of Object.entries(values)) out.push([`${group}.${name}`, String(value)]);
  }
  return out;
}

/** Every token of the design file as `[key, value]`, with the keys and text of `DESIGN.md`. */
export function allTokens(): [string, string][] {
  const out: [string, string][] = [];
  for (const t of THEMES) {
    for (const n of colorNames) out.push([`colour.${t}.${n}`, colors[t][n]]);
    for (const n of effectNames) out.push([`colour.${t}.${n}`, effects[t][n]]);
  }
  return [...out, ...scalarTokens()];
}

/** Every custom property that does not depend on the theme, as the CSS file must declare it. */
export function scalarVars(): Record<string, string> {
  const out: Record<string, string> = {
    [cssVar('font-family-sans')]: fonts.sans,
    [cssVar('font-family-mono')]: fonts.mono,
  };
  for (const [key, value] of scalarTokens().slice(2)) {
    const [group, name] = key.split('.') as [string, string];
    out[cssVar(`${group}-${name}`)] = `${value}${UNITS[group] ?? ''}`;
  }
  return out;
}

/** The colour custom properties of one theme (colours and effects). */
export function colorVars(theme: Theme): Record<string, string> {
  return Object.fromEntries([
    ...colorNames.map((n) => [cssVar(n), colors[theme][n]]),
    ...effectNames.map((n) => [cssVar(n), effects[theme][n]]),
  ]);
}
