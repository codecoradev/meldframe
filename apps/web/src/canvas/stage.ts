// The world layer: one Pixi display object per scene node, kept in step with the resolved scene by patches.
// The root is a render group and is the camera: pan and zoom are one matrix on it. Nodes are laid out in
// their parent's space (the resolved `lx`, `ly`), so a frame's clip mask and children follow it. Nodes take no
// pointer events (the canvas hit-tests the scene model itself), and display objects are culled by hand.
//
// A node is a container with, back to front: its shadows (a part for each, with a blur filter when it has a blur; not
// inside `content`, so the node's own clip does not cut them), its body, its text, `content` (the child nodes, which a clip
// mask cuts) and its border ring (over the children, and not cut by the node's own clip: the export clips the border of a
// clipping frame, the report says `border_clipped`, and the canvas does not).
import { Container, Culler, Graphics, RenderTexture, Sprite, Text, Texture, type Rectangle, type Renderer } from 'pixi.js';
import { borderRing, sameBorder } from '@meldframe/web-canvas/border';
import { sameGradient } from '@meldframe/web-canvas/gradient';
import { sameImage } from '@meldframe/web-canvas/image';
import { sameShadows } from '@meldframe/web-canvas/shadow';
import type { AssetCache } from '@meldframe/web-assets/assets';
import { paintBody, type Drawable } from './bodycanvas';
import type { Rect } from '@meldframe/web-canvas/geometry';
import type { Camera } from '@meldframe/web-canvas/camera';
import type { ResolvedNode, ResolvedScene, ScenePatch } from '@meldframe/web-canvas/scene';
import { alignOffset } from '@meldframe/web-canvas/textstyle';
import { drawStyle } from './pixitext';
import { dropShadow, paintShadows, tuneShadows, type ShadowPart } from './shadowparts';

type Body = Sprite | Graphics;

interface NodeView {
  c: Container;
  node: ResolvedNode;
  /** Behind the body, first = bottom; each with its own blur filter when it has a blur. */
  shadows: ShadowPart[];
  body: Body | null;
  text: Text | null;
  /** Holds the child nodes, and the clip mask first; made when the node first needs it. */
  content: Container | null;
  mask: Graphics | null;
  border: Graphics | null;
  /** The resolution the text was last rasterised at. */
  textRes: number;
}

/** Text is rasterised at most this many times the device pixel ratio's worth of zoom. */
const MAX_TEXT_RES = 4;

export class WorldView {
  readonly root = new Container({ isRenderGroup: true, label: 'world' });
  private views = new Map<string, NodeView>();
  /** The scale of the world as the camera has it; a blur filter works in screen pixels, so its strength follows it. */
  private zoom = 1;

  /** Where the images of image fills come from; without it a node draws its fill and gradient only. */
  constructor(private assets: AssetCache | null = null) {
    this.root.eventMode = 'none';
  }

  get nodeCount(): number {
    return this.views.size;
  }

  /** How many node containers are currently drawn (visible, not culled, no ancestor culled); for measurements. */
  countRenderable(): number {
    const memo = new Map<string, boolean>();
    const drawn = (id: string): boolean => {
      const hit = memo.get(id);
      if (hit !== undefined) return hit;
      const v = this.views.get(id)!;
      const ok = !v.c.culled && v.c.visible && (v.node.parent === null || drawn(v.node.parent));
      memo.set(id, ok);
      return ok;
    };
    let n = 0;
    for (const id of this.views.keys()) if (drawn(id)) n++;
    return n;
  }

  setCamera(cam: Camera): void {
    this.root.position.set(cam.x, cam.y);
    this.root.scale.set(cam.zoom);
    this.retune(cam.zoom);
  }

  /** Sets the blur of every shadow for a world scaled by `zoom`. */
  private retune(zoom: number): void {
    if (zoom === this.zoom) return;
    this.zoom = zoom;
    for (const v of this.views.values()) tuneShadows(v.shadows, zoom);
  }

  apply(rs: ResolvedScene, patch: ScenePatch): void {
    const removed = new Set(patch.removed);
    // First free the bodies of every removed node: a parent destroyed with `children: true` destroys the container of a
    // removed child too, and then the child's gradient texture could no longer be reached.
    for (const id of patch.removed) {
      const v = this.views.get(id);
      if (v && !v.c.destroyed) this.dropBody(v);
      if (v) this.dropShadows(v);
    }
    for (const id of patch.removed) {
      const v = this.views.get(id);
      this.views.delete(id);
      if (!v || v.c.destroyed) continue;
      // A child that survives (moved to another parent in the same patch) must not die with its old parent:
      // detach it first, the later pass puts it under its new parent.
      for (const child of [...(v.content?.children ?? [])]) {
        const keep = child.label !== null && this.views.get(child.label)?.c === child && !removed.has(child.label);
        if (keep) v.content!.removeChild(child);
      }
      v.c.destroy({ children: true });
    }
    let reparented = false;
    for (const id of patch.added) {
      const n = rs.byId.get(id)!;
      const c = new Container({ label: id });
      c.eventMode = 'none';
      c.cullable = true;
      const v: NodeView = { c, node: n, shadows: [], body: null, text: null, content: null, mask: null, border: null, textRes: 1 };
      this.views.set(id, v);
      this.update(v, n, true);
    }
    for (const id of patch.changed) {
      const v = this.views.get(id)!;
      const n = rs.byId.get(id)!;
      if (v.node.parent !== n.parent) reparented = true;
      this.update(v, n, false);
    }
    if (patch.added.length || reparented || patch.orderChanged) this.sortChildren(rs);
  }

  /** Where the child nodes of `parent` go: the canvas itself, or the content of the node. */
  private parentContainer(parent: string | null): Container {
    if (parent === null) return this.root;
    const v = this.views.get(parent)!;
    if (!v.content) {
      v.content = new Container({ label: 'content' });
      v.content.eventMode = 'none';
      this.placeParts(v);
    }
    return v.content;
  }

  /** The parts of a node that come before its child nodes in their container: the clip mask. */
  private fixedCount(parent: string | null): number {
    return parent === null ? 0 : this.views.get(parent)!.mask ? 1 : 0;
  }

  /** Puts every node under its parent, after the parent's own parts, in paint order. */
  private sortChildren(rs: ResolvedScene): void {
    const next = new Map<string | null, number>();
    for (const n of rs.items) {
      const v = this.views.get(n.id)!;
      const parent = this.parentContainer(n.parent);
      const i = (next.get(n.parent) ?? this.fixedCount(n.parent));
      next.set(n.parent, i + 1);
      if (v.c.parent !== parent) parent.addChildAt(v.c, Math.min(i, parent.children.length));
      else if (parent.children[i] !== v.c) parent.setChildIndex(v.c, i);
    }
  }

  private update(v: NodeView, n: ResolvedNode, first: boolean): void {
    const prev = v.node;
    v.node = n;
    v.c.position.set(n.lx, n.ly);
    v.c.visible = n.visible;
    v.c.alpha = n.opacity;
    const geometry = first || prev.w !== n.w || prev.h !== n.h || prev.radius !== n.radius || prev.kind !== n.kind;
    const fill = first || prev.fill !== n.fill || prev.fillAlpha !== n.fillAlpha || !sameGradient(prev.gradient, n.gradient) || !sameImage(prev.image, n.image);
    const cast = first || geometry || !sameShadows(prev.shadows, n.shadows);
    const ring = first || geometry || !sameBorder(prev.border, n.border);
    if (n.kind === 'text') this.updateText(v, n, first || geometry || fill || prev.text !== n.text || prev.fontSize !== n.fontSize || prev.fontWeight !== n.fontWeight || prev.textAlign !== n.textAlign);
    else if (geometry || fill) this.updateBody(v, n);
    if (first || prev.clip !== n.clip || (n.clip && geometry)) this.updateMask(v, n);
    if (cast) v.shadows = paintShadows(v.shadows, n, this.zoom);
    if (ring) this.updateBorder(v, n);
    this.placeParts(v);
  }

  /** Puts the parts in their order: shadows, body, text, content and ring in the node; the mask first in the content. */
  private placeParts(v: NodeView): void {
    let i = 0;
    for (const part of [...v.shadows.map((p) => p.g), v.body, v.text, v.content, v.border]) if (part) v.c.addChildAt(part, i++);
    if (v.content && v.mask) v.content.addChildAt(v.mask, 0);
  }

  /** Removes the body; a gradient's texture is its own and goes with it (the white texture is shared and stays). */
  private dropBody(v: NodeView): void {
    const b = v.body;
    if (b instanceof Sprite && b.texture !== Texture.WHITE) b.destroy({ texture: true, textureSource: true });
    else b?.destroy();
    v.body = null;
  }

  /** Removes the shadows with their filters. */
  private dropShadows(v: NodeView): void {
    for (const p of v.shadows) if (!p.g.destroyed) dropShadow(p);
    v.shadows = [];
  }

  /** Draws again every node whose image is the asset `id`, now that it has arrived (or failed). */
  repaintAsset(id: string): void {
    for (const v of this.views.values()) {
      if (v.node.image?.assetId !== id || v.node.kind === 'text') continue;
      this.updateBody(v, v.node);
      this.placeParts(v);
    }
  }

  private updateBody(v: NodeView, n: ResolvedNode): void {
    const plain = n.radius <= 0;
    // The image is there once the asset has arrived; until then (and for a missing one) the node shows its fill.
    const asset = n.image && this.assets ? this.assets.want(n.image.assetId) : null;
    const image = n.image && asset?.state === 'ready' ? { bitmap: asset.bitmap as Drawable, fit: n.image.fit } : undefined;
    if ((n.fill === null && !n.gradient && !image) || n.w <= 0 || n.h <= 0) {
      this.dropBody(v);
      return;
    }
    if (n.gradient || image) {
      // The fill, the gradient over it, the image over both and the rounded corners are one texture (see bodycanvas.ts).
      this.dropBody(v);
      const { canvas, k } = paintBody({
        w: n.w, h: n.h, radius: n.radius,
        fill: n.fill === null ? null : `#${n.fill.toString(16).padStart(6, '0')}`,
        fillAlpha: n.fillAlpha,
        gradient: n.gradient,
        image,
      });
      const s = new Sprite(Texture.from(canvas));
      s.eventMode = 'none';
      s.scale.set(1 / k);
      v.body = s;
      return;
    }
    if (plain) {
      let s = v.body instanceof Sprite && v.body.texture === Texture.WHITE ? v.body : null;
      if (!s) {
        this.dropBody(v);
        s = new Sprite(Texture.WHITE);
        s.eventMode = 'none';
        v.body = s;
      }
      s.tint = n.fill ?? 0;
      s.alpha = n.fillAlpha;
      s.width = n.w;
      s.height = n.h;
      return;
    }
    let g = v.body instanceof Graphics ? v.body : null;
    if (!g) {
      this.dropBody(v);
      g = new Graphics();
      g.eventMode = 'none';
      v.body = g;
    }
    g.clear().roundRect(0, 0, n.w, n.h, Math.min(n.radius, n.w / 2, n.h / 2)).fill({ color: n.fill ?? 0, alpha: n.fillAlpha });
  }

  private updateText(v: NodeView, n: ResolvedNode, redo: boolean): void {
    if (!redo && v.text) return;
    this.dropBody(v);
    const style = drawStyle(n);
    if (!v.text) {
      v.text = new Text({ text: n.text, style });
      v.text.eventMode = 'none';
    } else {
      v.text.text = n.text;
      v.text.style = style;
    }
    // The block of lines is as wide as its widest line: put it where the alignment says in the box.
    v.text.alpha = n.fillAlpha; // the alpha of the text's own colour; the container's alpha is the node's opacity
    v.text.x = alignOffset(n.textAlign, n.w, v.text.width);
  }

  private updateMask(v: NodeView, n: ResolvedNode): void {
    if (!n.clip) {
      if (v.content) v.content.mask = null;
      v.mask?.destroy();
      v.mask = null;
      return;
    }
    const g = v.mask ?? new Graphics();
    // Any opaque fill will do: only the shape of a mask counts.
    g.clear().rect(0, 0, n.w, n.h).fill(0);
    v.mask = g;
    if (!v.content) this.parentContainer(n.id);
    v.content!.mask = g;
  }

  /** The border: a ring over the node and its children, the outer shape with the inner one cut out (see `borderRing`). */
  private updateBorder(v: NodeView, n: ResolvedNode): void {
    const b = n.border;
    // A box with no width or height still has its ring (a divider); a size that is not a number has none.
    if (!b || n.kind === 'text' || !Number.isFinite(n.w) || !Number.isFinite(n.h) || n.w < 0 || n.h < 0) {
      v.border?.destroy();
      v.border = null;
      return;
    }
    const g = v.border ?? new Graphics();
    g.eventMode = 'none';
    const { outer, inner } = borderRing(n.w, n.h, n.radius, b);
    g.clear().roundRect(outer.x, outer.y, outer.w, outer.h, outer.r).fill({ color: b.color, alpha: b.alpha });
    if (inner) g.roundRect(inner.x, inner.y, inner.w, inner.h, inner.r).cut();
    v.border = g;
  }

  /** Hides every node outside `view` (screen px, through the camera); call when pan or zoom settles. */
  cull(view: Rectangle | { x: number; y: number; width: number; height: number }): void {
    Culler.shared.cull(this.root, view, false);
  }

  /** Re-rasterises visible text at the sharpness the zoom needs (it would blur when zoomed in otherwise). */
  sharpenText(zoom: number, dpr: number): void {
    const want = Math.min(MAX_TEXT_RES, Math.max(1, dpr * zoom));
    for (const v of this.views.values()) {
      if (!v.text || v.c.culled) continue;
      if (v.textRes / want < 1.34 && want / v.textRes < 1.34) continue;
      v.text.resolution = want;
      v.textRes = want;
    }
  }

  /** Draws the text again, for a web font that arrived after the first draw. */
  refreshText(): void {
    for (const v of this.views.values()) if (v.text) this.updateText(v, v.node, true);
  }

  /**
   * Draws the world through a camera of its own into an image of `width` x `height` pixels: `box` (world units) at `scale`.
   * Nothing is culled, and the view's camera is put back. The overlay (selection, handles, rings) is another layer and is
   * not in the image. The caller redraws the view afterwards.
   */
  renderRegion(renderer: Renderer, box: Rect, scale: number, width: number, height: number): HTMLCanvasElement {
    const rt = RenderTexture.create({ width, height, resolution: 1 });
    const { x, y } = this.root.position;
    const zoom = this.root.scale.x;
    const was = this.zoom;
    try {
      for (const v of this.views.values()) v.c.culled = false;
      // The blur of a shadow is in screen pixels: it takes the scale of the picture, not the camera's.
      this.retune(scale);
      this.root.position.set(-box.x * scale, -box.y * scale);
      this.root.scale.set(scale);
      renderer.render({ container: this.root, target: rt, clear: true, clearColor: [0, 0, 0, 0] });
      return renderer.extract.canvas(rt) as HTMLCanvasElement;
    } finally {
      this.root.position.set(x, y);
      this.root.scale.set(zoom);
      this.retune(was);
      rt.destroy(true);
    }
  }

  destroy(): void {
    for (const v of this.views.values()) {
      this.dropBody(v);
      this.dropShadows(v);
    }
    this.root.destroy({ children: true });
    this.views.clear();
  }
}
