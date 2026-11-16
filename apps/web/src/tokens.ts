// The one place for colour, font and size literals of the web client (U4). Everything else reads from
// here: components through CSS custom properties (`applyTokens`), the canvas through `tokens` directly.
// `node scripts/ui-literals.mjs` fails on a literal anywhere else. The values are placeholders until
// the design file exists, like crates/ui/meldframe-ui/src/tokens.rs.

export const tokens = {
  colour: {
    bg: '#16181d',
    panel: '#1e2128',
    border: '#2d313a',
    text: '#e6e8ec',
    textMuted: '#9097a3',
    accent: '#4c8dff',
    agent: '#e0a030',
    canvas: '#101216',
    nodeFrame: '#2a2f3a',
    nodeText: '#e6e8ec',
    selection: '#4c8dff',
    danger: '#ff6b6b',
  },
  font: { ui: 'system-ui, sans-serif', canvas: 'sans-serif' },
  fontSize: { sm: 11, md: 13 },
  space: { 1: 4, 2: 8, 3: 12 },
  size: {
    panelWidth: 240,
    rowHeight: 26,
    toolbarHeight: 40,
    radius: 4,
    border: 1,
    indent: 14,
    selectionStroke: 2,
    frameStroke: 1,
    inputWidth: 64,
  },
  duration: { fast: 120 },
} as const;

/** The CSS custom properties for `tokens`: `--colour-text-muted`, `--space-2` (px), `--duration-fast` (ms). */
export function cssVars(t: typeof tokens = tokens): Record<string, string> {
  const kebab = (s: string) => s.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`);
  const out: Record<string, string> = {};
  for (const [group, values] of Object.entries(t)) {
    for (const [key, v] of Object.entries(values)) {
      const unit = group === 'duration' ? 'ms' : typeof v === 'number' ? 'px' : '';
      out[`--${kebab(group)}-${kebab(String(key))}`] = `${v}${unit}`;
    }
  }
  return out;
}

export function applyTokens(root: HTMLElement = document.documentElement): void {
  for (const [name, value] of Object.entries(cssVars())) root.style.setProperty(name, value);
}
