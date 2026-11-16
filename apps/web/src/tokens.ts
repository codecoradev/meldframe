// The tokens module of the web client: the single source of every colour, type size, spacing step,
// radius, shadow and height (U4). `tokens.css` mirrors it and `tokens.test.ts` fails when the two
// drift. The PixiJS canvas reads the numeric colours from `pixiColors`.
// Values come from the owner's Foundations design (dark first, light the second set, same names).

export const THEMES = ['dark', 'light'] as const;
export type Theme = (typeof THEMES)[number];

const dark = {
  canvas: '#161618',
  sidebar: '#1B1B1E',
  panel: '#1E1E21',
  field: '#26262A',
  raised: '#26262B',
  hairline: '#2E2E33',
  'menu-border': '#3A3A42',
  'text-strong': '#F4F4F5',
  text: '#E4E4E7',
  'text-secondary': '#A1A1AA',
  'text-muted': '#8B8B94',
  accent: '#3B82F6',
  primary: '#2563EB',
  'on-primary': '#FFFFFF',
  selection: '#60A5FA',
  'selection-bg': '#1F2540',
  'chip-bg': '#1F2540',
  'chip-border': '#343D6E',
  'chip-text': '#C7D2FE',
  'actor-you': '#7C6CF0',
  'actor-agent-1': '#FB923C',
  'actor-agent-2': '#2DD4BF',
  'actor-agent-3': '#C084FC',
  'on-agent-1': '#1C1917',
  success: '#4ADE80',
  danger: '#F87171',
  'danger-solid': '#B91C1C',
  warning: '#FDE68A',
  info: '#93C5FD',
} as const;

export type ColorName = keyof typeof dark;

const light: Record<ColorName, string> = {
  canvas: '#E9E9EC',
  sidebar: '#F0F0F3',
  panel: '#FFFFFF',
  field: '#F4F4F5',
  raised: '#FFFFFF',
  hairline: '#E4E4E7',
  'menu-border': '#D4D4D8',
  'text-strong': '#09090B',
  text: '#18181B',
  'text-secondary': '#52525B',
  'text-muted': '#52525B',
  accent: '#2563EB',
  primary: '#2563EB',
  'on-primary': '#FFFFFF',
  selection: '#2563EB',
  'selection-bg': '#DBEAFE',
  'chip-bg': '#E0E7FF',
  'chip-border': '#C7D2FE',
  'chip-text': '#3730A3',
  'actor-you': '#7C6CF0',
  'actor-agent-1': '#FB923C',
  'actor-agent-2': '#2DD4BF',
  'actor-agent-3': '#C084FC',
  'on-agent-1': '#1C1917',
  success: '#15803D',
  danger: '#B91C1C',
  'danger-solid': '#B91C1C',
  warning: '#A16207',
  info: '#1D4ED8',
};

/** Light values the owner's design does not give; derived here and to be confirmed by the owner. */
export const LIGHT_DERIVED: readonly ColorName[] = [
  'sidebar', 'raised', 'menu-border', 'text-strong', 'text-secondary', 'selection', 'chip-bg',
  'chip-border', 'chip-text', 'success', 'danger', 'warning', 'info',
];

export const colors: Record<Theme, Record<ColorName, string>> = { dark, light };
export const colorNames = Object.keys(dark) as ColorName[];

/** `#RRGGBB` as the number PixiJS takes. */
export const hexToNumber = (hex: string): number => Number.parseInt(hex.slice(1), 16);

/** Numeric colours of one theme, for the canvas. */
export function pixiColors(theme: Theme): Record<ColorName, number> {
  return Object.fromEntries(colorNames.map((n) => [n, hexToNumber(colors[theme][n])])) as Record<ColorName, number>;
}

export const fonts = {
  ui: "'Inter Variable', Inter, system-ui, sans-serif",
  mono: "'JetBrains Mono Variable', 'JetBrains Mono', ui-monospace, monospace",
} as const;

/** Type scale in px with the weights the design allows. */
export const typeScale = {
  title: { size: 24, weights: [700] },
  dialog: { size: 22, weights: [600] },
  panel: { size: 16, weights: [600] },
  base: { size: 13, weights: [400, 500, 600] },
  dense: { size: 12, weights: [400, 500] },
  label: { size: 11, weights: [600] },
} as const;

export const space = [4, 8, 12, 16, 24, 32] as const;
export const radii = { button: 7, field: 8, panel: 12, pill: 999 } as const;
export const heights = { button: 28, field: 28, chip: 22, 'menu-row': 30 } as const;
export const shadows = {
  float: '0 8px 24px rgba(0, 0, 0, 0.35)',
  popover: '0 12px 32px rgba(0, 0, 0, 0.5)',
} as const;

export const cssVar = (name: string): string => `--mf-${name}`;

/** Every custom property that does not depend on the theme, as the CSS file must declare it. */
export function scalarVars(): Record<string, string> {
  const out: Record<string, string> = {
    [cssVar('font-ui')]: fonts.ui,
    [cssVar('font-mono')]: fonts.mono,
  };
  for (const [k, v] of Object.entries(typeScale)) out[cssVar(`text-${k}`)] = `${v.size}px`;
  for (const s of space) out[cssVar(`space-${s}`)] = `${s}px`;
  for (const [k, v] of Object.entries(radii)) out[cssVar(`radius-${k}`)] = `${v}px`;
  for (const [k, v] of Object.entries(heights)) out[cssVar(`height-${k}`)] = `${v}px`;
  for (const [k, v] of Object.entries(shadows)) out[cssVar(`shadow-${k}`)] = v;
  return out;
}

/** The colour custom properties of one theme. */
export function colorVars(theme: Theme): Record<string, string> {
  return Object.fromEntries(colorNames.map((n) => [cssVar(n), colors[theme][n]]));
}
