// WCAG 2.x contrast ratio between two `#RRGGBB` colours, for the token contrast test.

function channel(v: number): number {
  const c = v / 255;
  return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

function luminance(hex: string): number {
  const n = Number.parseInt(hex.slice(1), 16);
  return 0.2126 * channel((n >> 16) & 255) + 0.7152 * channel((n >> 8) & 255) + 0.0722 * channel(n & 255);
}

/** Ratio from 1 to 21. */
export function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x) as [number, number];
  return (hi + 0.05) / (lo + 0.05);
}

/** The colour a translucent `rgba(r,g,b,a)` (or a plain `#RRGGBB`) gives over an opaque `#RRGGBB` base, as `#RRGGBB`. */
export function over(color: string, base: string): string {
  const m = /^rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*(?:,\s*([\d.]+)\s*)?\)$/.exec(color.trim());
  if (!m) return color;
  const a = m[4] === undefined ? 1 : Number(m[4]);
  const b = Number.parseInt(base.slice(1), 16);
  const mix = (fg: number, bg: number) => Math.round(fg * a + bg * (1 - a));
  const rgb = [mix(Number(m[1]), (b >> 16) & 255), mix(Number(m[2]), (b >> 8) & 255), mix(Number(m[3]), b & 255)];
  return `#${rgb.map((c) => c.toString(16).padStart(2, '0')).join('')}`;
}
