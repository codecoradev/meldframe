// The shadows of a node as Pixi parts: one Graphics for each shadow, in the model's order, with a blur filter when the
// shadow has a blur. Kept out of `stage.ts`, which places them. The shape is `shadowShape` (the rule of a CSS `box-shadow`).
//
// A filter works in screen pixels, and the world is scaled by the camera (and by the scale of a capture), so the blur of a
// world radius `b` is a strength of `b / 2 * zoom` (a Gaussian of sigma `b / 2` is what CSS specifies): measured in Chromium,
// within 19/255 of the CSS blur at zoom 0.25, 0.5, 1 and 2 with that rule, and 26 to 58/255 off at zoom 2 and 0.5 without it.
import { BlurFilter, Graphics } from 'pixi.js';
import { shadowShape } from '@meldframe/web-canvas/shadow';
import type { ResolvedNode } from '@meldframe/web-canvas/scene';

/** The passes of the blur filter: the oracle measures the canvas blur at this quality (contract, section 8.5). The filter keeps its default resolution (1): a blur is smooth, and on a 2x screen it is the same blur in CSS pixels, measured. */
const BLUR_QUALITY = 4;

export interface ShadowPart {
  g: Graphics;
  /** The blur radius in world units; 0 for a hard edge. */
  blur: number;
}

/** Sets the filter of a part to the strength a camera at `zoom` needs. */
function tune(p: ShadowPart, zoom: number): void {
  const f = p.g.filters?.[0] as BlurFilter | undefined;
  if (!f) return;
  f.strength = (p.blur / 2) * zoom;
  // Room for the tail of the blur, which is why the filter has to be told: it is clipped to its padded bounds.
  f.padding = Math.ceil(1.5 * p.blur * zoom) + 2;
}

/** Frees a part and its filter. */
export function dropShadow(p: ShadowPart): void {
  for (const f of p.g.filters ?? []) (f as BlurFilter).destroy();
  p.g.filters = null;
  p.g.destroy();
}

/** The shadows of `n` as parts, reusing `old` where it can: the same count and order draws into the same Graphics. */
export function paintShadows(old: readonly ShadowPart[], n: ResolvedNode, zoom: number): ShadowPart[] {
  const finite = Number.isFinite(n.w) && Number.isFinite(n.h);
  const shapes = n.kind === 'text' || !finite
    ? []
    : (n.shadows ?? []).flatMap((s) => {
        const shape = shadowShape(n.w, n.h, n.radius, s);
        return shape ? [{ s, shape }] : [];
      });
  const out: ShadowPart[] = [];
  shapes.forEach(({ s, shape }, i) => {
    const part = old[i] ?? { g: new Graphics(), blur: 0 };
    const { g } = part;
    g.label = 'shadow';
    g.eventMode = 'none';
    g.clear().roundRect(shape.x, shape.y, shape.w, shape.h, shape.r).fill({ color: s.color, alpha: s.alpha });
    part.blur = s.blur;
    if (s.blur > 0 && !g.filters?.length) g.filters = [new BlurFilter({ strength: 1, quality: BLUR_QUALITY })];
    else if (!(s.blur > 0) && g.filters?.length) {
      for (const f of g.filters) (f as BlurFilter).destroy();
      g.filters = null;
    }
    tune(part, zoom);
    out.push(part);
  });
  for (const p of old.slice(out.length)) dropShadow(p);
  return out;
}

/** Sets the blur of every part that has one for a camera at `zoom`. */
export function tuneShadows(parts: readonly ShadowPart[], zoom: number): void {
  for (const p of parts) tune(p, zoom);
}
