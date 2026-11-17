// The world layer: one Pixi display object per scene node, kept in step with the resolved scene by patches.
// The root is a render group and is the camera: pan and zoom are one matrix on it. Nodes are laid out in
// their parent's space (the resolved `lx`, `ly`), so a frame's clip mask and children follow it. Nodes take no
// pointer events (the canvas hit-tests the scene model itself), and display objects are culled by hand.
import { Container, Culler, Graphics, RenderTexture, Sprite, Text, Texture, type Rectangle, type Renderer } from 'pixi.js';
import type { Rect } from '@meldframe/web-canvas/geometry';
import type { Camera } from '@meldframe/web-canvas/camera';
import type { ResolvedNode, ResolvedScene, ScenePatch } from '@meldframe/web-canvas/scene';
import { fonts } from '../tokens';

type Body = Sprite | Graphics;

interface NodeView {
  c: Container;
  node: ResolvedNode;
  body: Body | null;
  text: Text | null;
  mask: Graphics | null;
  /** The resolution the text was last rasterised at. */
  textRes: number;
}

/** Text is rasterised at most this many times the device pixel ratio's worth of zoom. */
const MAX_TEXT_RES = 4;

export class WorldView {
  readonly root = new Container({ isRenderGroup: true, label: 'world' });
  private views = new Map<string, NodeView>();

  constructor() {
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
  }

  apply(rs: ResolvedScene, patch: ScenePatch): void {
    const removed = new Set(patch.removed);
    for (const id of patch.removed) {
      const v = this.views.get(id);
      this.views.delete(id);
      if (!v || v.c.destroyed) continue;
      // A child that survives (moved to another parent in the same patch) must not die with its old parent:
      // detach it first, the later pass puts it under its new parent.
      for (const child of [...v.c.children]) {
        const keep = child.label !== null && this.views.get(child.label)?.c === child && !removed.has(child.label);
        if (keep) v.c.removeChild(child);
      }
      v.c.destroy({ children: true });
    }
    let reparented = false;
    for (const id of patch.added) {
      const n = rs.byId.get(id)!;
      const c = new Container({ label: id });
      c.eventMode = 'none';
      c.cullable = true;
      const v: NodeView = { c, node: n, body: null, text: null, mask: null, textRes: 1 };
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

  private parentContainer(parent: string | null): Container {
    return parent === null ? this.root : this.views.get(parent)!.c;
  }

  private fixedCount(parent: string | null): number {
    if (parent === null) return 0;
    const v = this.views.get(parent)!;
    return (v.body ? 1 : 0) + (v.text ? 1 : 0) + (v.mask ? 1 : 0);
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
    const fill = first || prev.fill !== n.fill;
    if (n.kind === 'text') this.updateText(v, n, first || geometry || fill || prev.text !== n.text || prev.fontSize !== n.fontSize);
    else if (geometry || fill) this.updateBody(v, n);
    if (first || prev.clip !== n.clip || (n.clip && geometry)) this.updateMask(v, n);
    this.placeParts(v);
  }

  private placeParts(v: NodeView): void {
    let i = 0;
    for (const part of [v.body, v.text, v.mask]) if (part) v.c.addChildAt(part, i++);
  }

  private updateBody(v: NodeView, n: ResolvedNode): void {
    const plain = n.radius <= 0;
    if (n.fill === null || n.w <= 0 || n.h <= 0) {
      v.body?.destroy();
      v.body = null;
      return;
    }
    if (plain) {
      let s = v.body instanceof Sprite ? v.body : null;
      if (!s) {
        v.body?.destroy();
        s = new Sprite(Texture.WHITE);
        s.eventMode = 'none';
        v.body = s;
      }
      s.tint = n.fill;
      s.width = n.w;
      s.height = n.h;
      return;
    }
    let g = v.body instanceof Graphics ? v.body : null;
    if (!g) {
      v.body?.destroy();
      g = new Graphics();
      g.eventMode = 'none';
      v.body = g;
    }
    g.clear().roundRect(0, 0, n.w, n.h, Math.min(n.radius, n.w / 2, n.h / 2)).fill(n.fill);
  }

  private updateText(v: NodeView, n: ResolvedNode, redo: boolean): void {
    if (!redo && v.text) return;
    v.body?.destroy();
    v.body = null;
    const style = {
      fontFamily: fonts.sans,
      fontSize: Math.max(1, n.fontSize),
      fill: n.fill ?? 0,
      wordWrap: n.w > 0,
      wordWrapWidth: Math.max(1, n.w),
    };
    if (!v.text) {
      v.text = new Text({ text: n.text, style });
      v.text.eventMode = 'none';
    } else {
      v.text.text = n.text;
      v.text.style = style;
    }
  }

  private updateMask(v: NodeView, n: ResolvedNode): void {
    if (!n.clip) {
      v.c.mask = null;
      v.mask?.destroy();
      v.mask = null;
      return;
    }
    const g = v.mask ?? new Graphics();
    // Any opaque fill will do: only the shape of a mask counts.
    g.clear().rect(0, 0, n.w, n.h).fill(0);
    v.mask = g;
    v.c.mask = g;
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
    try {
      for (const v of this.views.values()) v.c.culled = false;
      this.root.position.set(-box.x * scale, -box.y * scale);
      this.root.scale.set(scale);
      renderer.render({ container: this.root, target: rt, clear: true, clearColor: [0, 0, 0, 0] });
      return renderer.extract.canvas(rt) as HTMLCanvasElement;
    } finally {
      this.root.position.set(x, y);
      this.root.scale.set(zoom);
      rt.destroy(true);
    }
  }

  destroy(): void {
    this.root.destroy({ children: true });
    this.views.clear();
  }
}
