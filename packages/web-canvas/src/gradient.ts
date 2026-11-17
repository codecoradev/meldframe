// Gradients on the canvas: the line a linear gradient runs along and the radius of a radial one, worked out as CSS does,
// so the canvas and the export agree. Pure.

export interface SceneGradient {
  kind: 'linear' | 'radial';
  /** Degrees, as CSS: 0 up, 90 right, 180 down. A radial gradient ignores it. */
  angle: number;
  stops: readonly { pos: number; color: string }[];
}

/** The start and end of a linear gradient across a `w` x `h` box (box coordinates, y down), through the centre. */
export function gradientLine(angle: number, w: number, h: number): { x0: number; y0: number; x1: number; y1: number } {
  const t = (angle * Math.PI) / 180;
  const dx = Math.sin(t);
  const dy = -Math.cos(t);
  // CSS: the line is as long as the box is wide along the direction, so the corners get the first and last colour.
  const half = (Math.abs(w * dx) + Math.abs(h * dy)) / 2;
  const cx = w / 2;
  const cy = h / 2;
  return { x0: cx - dx * half, y0: cy - dy * half, x1: cx + dx * half, y1: cy + dy * half };
}

/** The radius of a radial gradient that reaches the farthest corner from the centre. */
export const radialRadius = (w: number, h: number): number => Math.hypot(w / 2, h / 2);

export function sameGradient(a: SceneGradient | undefined, b: SceneGradient | undefined): boolean {
  if (a === b) return true;
  if (!a || !b) return false;
  return a.kind === b.kind && a.angle === b.angle && a.stops.length === b.stops.length &&
    a.stops.every((s, i) => s.pos === b.stops[i]!.pos && s.color === b.stops[i]!.color);
}
