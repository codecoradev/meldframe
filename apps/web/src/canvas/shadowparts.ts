// The shadows of a node as Pixi parts: one Graphics for each shadow, in the model's order, with a blur filter when the
// shadow has a blur, in a layer of their own that is masked so that the node's own box is cut out of them. Kept out of `stage.ts`,
// which places the layer. The shape is `shadowShape` (the rule of a CSS `box-shadow`).
//
// CSS draws no `box-shadow` inside the border box, so the layer has a stencil mask whose shape is a rectangle that holds every
// shadow with its blur, with the box (its rounded rectangle) cut out as a hole (the same cut as the border ring). The stencil is
// applied where the layer's filtered parts are composited, that is after the blur: a hole in the shape before the blur would blur
// the hole. A mask that is not inverse is the one that nests under the stencil of a clipping ancestor (Pixi's inverse mask lets
// pixels outside the ancestor's clip through).
//
// A filter works in screen pixels, and the world is scaled by the camera (and by the scale of a capture), so the blur of a
// world radius `b` is a strength of `b / 2 * zoom` (a Gaussian of sigma `b / 2` is what CSS specifies). Without the factor the
// blur is far from CSS's at any zoom but 1; the measurements are in the contract (the row `shadows`, and section 7).
import { BlurFilter, Container, Graphics, Rectangle } from 'pixi.js';
import { blurFilterSettings, shadowShape, type ShadowShape } from '@meldframe/web-canvas/shadow';
import type { ResolvedNode } from '@meldframe/web-canvas/scene';

/** The passes of the blur filter: the oracle measures the canvas blur at this quality (contract, section 8.5). The filter keeps its default resolution (1): a blur is smooth, and on a 2x screen it is the same blur in CSS pixels, measured. */
const BLUR_QUALITY = 4;

export interface ShadowPart {
  g: Graphics;
  /** The blur radius in world units; 0 for a hard edge. */
  blur: number;
  /** Stands for the halo of a blurred shadow in the bounds that the culling reads: an empty container with `boundsArea`; none for a hard edge. */
  halo: Container | null;
}

/** How far the halo of a blur reaches past its shape, in world units: 1.5 times the blur and 2 (the culling's hint, and the room the mask leaves). */
const haloOf = (blur: number): number => (blur > 0 && Number.isFinite(blur) ? Math.ceil(1.5 * blur) + 2 : 2);

/** The display objects of the parts, in the order they are drawn. */
const partObjects = (parts: readonly ShadowPart[]): Container[] => parts.flatMap((p) => (p.halo ? [p.g, p.halo] : [p.g]));

/** Sets the filter of a part to the strength a camera at `zoom` needs. */
function tune(p: ShadowPart, zoom: number): void {
  const f = p.g.filters?.[0] as BlurFilter | undefined;
  if (!f) return;
  // Room for the tail of the blur, which is why the filter has to be told: it is clipped to its padded bounds. Both are held to MAX_BLUR.
  const { strength, padding } = blurFilterSettings(p.blur, zoom);
  f.strength = strength;
  f.padding = padding;
}

/** Frees a part and its filter. */
function dropPart(p: ShadowPart): void {
  for (const f of p.g.filters ?? []) (f as BlurFilter).destroy();
  p.g.filters = null;
  p.g.destroy();
  p.halo?.destroy();
}

/** The shadows of one node: the container that goes in the node, the parts in it and the mask that cuts the box out of them. */
export class ShadowLayer {
  readonly c = new Container({ label: 'shadows' });
  parts: ShadowPart[] = [];
  private cut: Graphics | null = null;

  constructor() {
    this.c.eventMode = 'none';
  }

  /** Whether a part has a blur (the layer then needs its filters tuned to the zoom). */
  get blurred(): boolean {
    return this.parts.some((p) => p.blur > 0);
  }

  /** Sets the blur of every part that has one for a camera at `zoom`. */
  tune(zoom: number): void {
    for (const p of this.parts) tune(p, zoom);
  }

  /** Frees the parts with their filters, the mask and the container. */
  destroy(): void {
    for (const p of this.parts) if (!p.g.destroyed) dropPart(p);
    this.parts = [];
    this.c.mask = null;
    this.cut?.destroy();
    this.cut = null;
    this.c.destroy();
  }

  /** Draws the shadows of `n` again, reusing the parts and filters that fit: the same count and order draws into the same Graphics. False when `n` has none to draw (the layer is then to be destroyed). */
  paint(n: ResolvedNode, zoom: number): boolean {
    const finite = Number.isFinite(n.w) && Number.isFinite(n.h);
    const shapes = n.kind === 'text' || !finite
      ? []
      : (n.shadows ?? []).flatMap((s) => {
          // A shadow with an alpha of 0 paints nothing, so it gets neither a part nor a filter pass.
          const shape = s.alpha > 0 ? shadowShape(n.w, n.h, n.radius, s) : null;
          return shape ? [{ s, shape }] : [];
        });
    const old = this.parts;
    const out: ShadowPart[] = [];
    shapes.forEach(({ s, shape }, i) => {
      const part = old[i] ?? { g: new Graphics(), blur: 0, halo: null };
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
      // The culling reads the bounds of a node and ignores the padding of a filter, so a blurred shadow says how far its halo reaches: 1.5 times
      // the blur and 2, in world units, uncapped (it is only a hint for the culling, not what the filter pads). A node that has left the view but
      // whose halo has not is still drawn. It is said by an empty container beside the shadow, not by `boundsArea` on the Graphics (that breaks
      // the filter, which sizes itself from it: the blur vanishes at some zooms), and not by a `cullArea` on the node, which would cull a child
      // that overflows the node's box.
      if (s.blur > 0) {
        const h = haloOf(s.blur);
        part.halo ??= new Container({ label: 'shadow-halo' });
        part.halo.eventMode = 'none';
        part.halo.boundsArea = new Rectangle(shape.x - h, shape.y - h, shape.w + 2 * h, shape.h + 2 * h);
      } else if (part.halo) {
        part.halo.destroy();
        part.halo = null;
      }
      tune(part, zoom);
      out.push(part);
    });
    for (const p of old.slice(out.length)) dropPart(p);
    this.parts = out;
    if (!out.length) return false;
    this.place();
    this.cutBox(n, shapes.map(({ s, shape }) => ({ shape, halo: haloOf(s.blur) })));
    return true;
  }

  /** The parts in their order, then the mask (a child, as a clip mask is; it is not drawn). */
  private place(): void {
    let i = 0;
    for (const o of partObjects(this.parts)) this.c.addChildAt(o, i++);
  }

  /**
   * Cuts the box of `n` out of the layer: the mask is a rectangle that holds the box, every shadow and its halo, with the box's rounded
   * rectangle (its radius as the body draws it) cut out. A box with no area has nothing to cut out, and then the layer has no mask.
   */
  private cutBox(n: ResolvedNode, shapes: readonly { shape: ShadowShape; halo: number }[]): void {
    if (!(n.w > 0 && n.h > 0)) {
      this.c.mask = null;
      this.cut?.destroy();
      this.cut = null;
      return;
    }
    let x0 = 0;
    let y0 = 0;
    let x1 = n.w;
    let y1 = n.h;
    for (const { shape: sh, halo } of shapes) {
      x0 = Math.min(x0, sh.x - halo);
      y0 = Math.min(y0, sh.y - halo);
      x1 = Math.max(x1, sh.x + sh.w + halo);
      y1 = Math.max(y1, sh.y + sh.h + halo);
    }
    const g = this.cut ?? new Graphics();
    g.eventMode = 'none';
    // Any opaque fill will do: only the shape of a mask counts.
    g.clear().rect(x0, y0, x1 - x0, y1 - y0).fill(0).roundRect(0, 0, n.w, n.h, Math.min(n.radius, n.w / 2, n.h / 2)).cut();
    if (!this.cut) {
      this.cut = g;
      this.c.addChild(g);
      this.c.mask = g;
    }
  }
}
