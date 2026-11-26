// The overlay layer: selection outline, handles, size badge, hover outline, touch rings, frame names and
// the rubber band of a create tool. It is a plain container that the camera does not scale, so strokes stay
// 1.5 px and labels stay legible at any zoom; only positions go through the camera, each time `draw` runs.
// It takes no pointer events. Geometry comes from `overlay.ts`; this file only paints it.
import { Container, Graphics, Text } from 'pixi.js';
import { rectToScreen, type Camera } from '@meldframe/web-canvas/camera';
import { rectsOverlap, unionRects, type Rect, type Size } from '@meldframe/web-canvas/geometry';
import {
  HANDLE_SIZE, badgeOrigin, badgeText, handlePositions, selectionBox, slotSegment, touchAlpha, touchRing, visibleHandles,
} from '@meldframe/web-canvas/overlay';
import type { ResolvedScene } from '@meldframe/web-canvas/scene';
import { rectsOf } from '@meldframe/web-canvas/scene';
import { duration, fontSize, fontWeight, fonts, radius, stroke, type ColorName } from '@meldframe/web-tokens/tokens';

/** Touch ring timing from the tokens (the model package imports no app code). */
const RING_TIMING = { fadeMs: duration['touch-ring'], holdMs: duration['reduced-motion-hold'] };

/** An actor's mark on nodes: a ring in the actor's colour with a name label. */
export interface Touch {
  id: string;
  /** `0xRRGGBB` of the actor. */
  color: number;
  label: string;
  nodeIds: string[];
  /** When it happened, in `performance.now()` milliseconds. The canvas works out the fade from it. */
  at: number;
}

export type Palette = Record<ColorName, number>;

export interface OverlayState {
  camera: Camera;
  viewport: Size;
  scene: ResolvedScene;
  selection: readonly string[];
  hover: string | null;
  touches: readonly Touch[];
  /** Rubber band of a create tool, in world units. */
  draft: Rect | null;
  /** The line of the slot in a flow a dragged node would land in, in world units (zero thick on one axis). */
  indicator?: Rect | null;
  palette: Palette;
  reducedMotion: boolean;
  now: number;
}

const HOVER_ALPHA = 0.6;
const DRAFT_FILL_ALPHA = 0.12;
/// Names smaller than this on screen are not drawn.
const NAME_MIN_WIDTH = 24;
const NAME_GAP = 6;
const BADGE_PAD_X = 8;
const BADGE_PAD_Y = 3;
const TAG_PAD_X = 6;
const TAG_PAD_Y = 2;

function makeText(family: string, size: number, weight: number): Text {
  const t = new Text({ text: '', style: { fontFamily: family, fontSize: size, fontWeight: String(weight) as never, fill: 0 } });
  t.eventMode = 'none';
  return t;
}

export class OverlayView {
  readonly root = new Container({ label: 'overlay' });
  private g = new Graphics();
  private badgeBg = new Graphics();
  private badgeLabel = makeText(fonts.mono, fontSize.caption, fontWeight.medium);
  private tagBg = new Graphics();
  private tagText = makeText(fonts.sans, fontSize.caption, fontWeight.semibold);
  private names = new Map<string, Text>();
  private inks = new WeakMap<Text, number>();
  private nameKeys = new WeakMap<Text, string>();

  constructor() {
    this.root.eventMode = 'none';
    this.g.eventMode = 'none';
    this.root.addChild(this.g, this.badgeBg, this.badgeLabel, this.tagBg, this.tagText);
  }

  /** True when a touch is still visible at `now`, so the loop must keep drawing. */
  static animating(touches: readonly Touch[], now: number, reduced: boolean): boolean {
    return touches.some((t) => touchAlpha(now - t.at, reduced, RING_TIMING) > 0);
  }

  draw(s: OverlayState): void {
    const { camera: cam, viewport, palette: pal } = s;
    const g = this.g;
    g.clear();
    this.drawNames(s);

    if (s.draft) {
      const r = rectToScreen(cam, s.draft);
      g.rect(r.x, r.y, r.w, r.h).fill({ color: pal.accent, alpha: DRAFT_FILL_ALPHA }).stroke({ width: stroke.selection, color: pal.selection });
    }

    if (s.indicator) {
      const { from, to } = slotSegment(cam, s.indicator);
      g.moveTo(from.x, from.y).lineTo(to.x, to.y).stroke({ width: stroke.focus, color: pal.accent, cap: 'round' });
    }

    if (s.hover && !s.selection.includes(s.hover)) {
      const [r] = rectsOf(s.scene, [s.hover]);
      if (r) {
        const b = rectToScreen(cam, r);
        g.rect(b.x, b.y, b.w, b.h).stroke({ width: stroke.selection, color: pal.selection, alpha: HOVER_ALPHA, alignment: 1 });
      }
    }

    const box = selectionBox(cam, rectsOf(s.scene, s.selection));
    if (box) {
      g.rect(box.x, box.y, box.w, box.h).stroke({ width: stroke.selection, color: pal.selection, alignment: 1 });
      const pos = handlePositions(box);
      for (const h of visibleHandles(box)) {
        const p = pos[h];
        g.rect(p.x - HANDLE_SIZE / 2, p.y - HANDLE_SIZE / 2, HANDLE_SIZE, HANDLE_SIZE)
          .fill(pal['on-accent'])
          .stroke({ width: stroke.selection, color: pal.selection, alignment: 1 });
      }
      this.drawBadge(box, s);
    } else {
      this.badgeBg.clear();
      this.badgeLabel.visible = false;
    }
    this.drawTouches(s, viewport);
  }

  /** Sets a text colour only when it differs: a style write makes Pixi draw the text again. */
  private setInk(t: Text, color: number): void {
    if (this.inks.get(t) === color) return;
    this.inks.set(t, color);
    t.style.fill = color;
  }

  private drawBadge(box: Rect, s: OverlayState): void {
    const t = this.badgeLabel;
    const world = unionRects(rectsOf(s.scene, s.selection))!;
    const text = badgeText(world.w, world.h);
    if (t.text !== text) t.text = text;
    this.setInk(t, s.palette['on-accent']);
    const size = { w: t.width + 2 * BADGE_PAD_X, h: t.height + 2 * BADGE_PAD_Y };
    const o = badgeOrigin(box, size, s.viewport);
    this.badgeBg.clear().roundRect(o.x, o.y, size.w, size.h, radius.sm).fill(s.palette['accent-strong']);
    t.visible = true;
    t.position.set(o.x + BADGE_PAD_X, o.y + BADGE_PAD_Y);
  }

  private drawTouches(s: OverlayState, viewport: Size): void {
    const g = this.g;
    this.tagBg.clear();
    this.tagText.visible = false;
    let tagged = false;
    for (const touch of s.touches) {
      const alpha = touchAlpha(s.now - touch.at, s.reducedMotion, RING_TIMING);
      if (alpha <= 0) continue;
      let first = true;
      for (const r of rectsOf(s.scene, touch.nodeIds)) {
        const box = rectToScreen(s.camera, r);
        if (!rectsOverlap(box, { x: -64, y: -64, w: viewport.w + 128, h: viewport.h + 128 })) continue;
        const ring = touchRing(box);
        g.roundRect(ring.rect.x, ring.rect.y, ring.rect.w, ring.rect.h, ring.radius)
          .stroke({ width: ring.stroke, color: touch.color, alpha });
        if (first && !tagged) {
          first = false;
          tagged = true;
          this.drawTag(touch, ring.label.x, ring.label.y, alpha, s.palette['on-agent']);
        }
      }
    }
  }

  /** The actor's name label; only one label shows at a time to keep the layer cheap. */
  private drawTag(touch: Touch, x: number, bottom: number, alpha: number, ink: number): void {
    const t = this.tagText;
    if (t.text !== touch.label) t.text = touch.label;
    this.setInk(t, ink);
    const w = t.width + 2 * TAG_PAD_X;
    const h = t.height + 2 * TAG_PAD_Y;
    this.tagBg.roundRect(x, bottom - h, w, h, radius.xs).fill({ color: touch.color, alpha });
    t.visible = true;
    t.alpha = alpha;
    t.position.set(x + TAG_PAD_X, bottom - h + TAG_PAD_Y);
  }

  /** The name, cut with an ellipsis to the frame's width on screen so neighbours do not run into each other. */
  private setName(t: Text, name: string, width: number): void {
    const key = `${name}|${Math.round(width)}`;
    if (this.nameKeys.get(t) === key) return;
    this.nameKeys.set(t, key);
    t.text = name;
    if (t.width <= width) return;
    // Cut on characters (code points), not on UTF-16 units: a unit cut would leave half of an emoji.
    const chars = Array.from(name);
    let n = Math.floor((chars.length * width) / t.width);
    do {
      t.text = `${chars.slice(0, Math.max(1, n)).join('')}\u2026`;
      n--;
    } while (t.width > width && n > 0);
  }

  private drawNames(s: OverlayState): void {
    const live = new Set<string>();
    for (const n of s.scene.items) {
      if (n.depth !== 0 || n.kind !== 'frame' || !n.name || !n.visible) continue;
      const b = rectToScreen(s.camera, n);
      let t = this.names.get(n.id);
      const onScreen = b.w >= NAME_MIN_WIDTH && rectsOverlap(b, { x: 0, y: -32, w: s.viewport.w, h: s.viewport.h + 32 });
      if (!onScreen) {
        if (t) t.visible = false;
        live.add(n.id);
        continue;
      }
      if (!t) {
        t = makeText(fonts.sans, fontSize.caption, fontWeight.regular);
        this.names.set(n.id, t);
        this.root.addChild(t);
      }
      live.add(n.id);
      this.setName(t, n.name, b.w);
      this.setInk(t, s.palette['text-muted']);
      t.visible = true;
      t.position.set(b.x, b.y - NAME_GAP - t.height);
    }
    for (const [id, t] of this.names) {
      if (live.has(id)) continue;
      t.destroy();
      this.names.delete(id);
    }
  }

  destroy(): void {
    this.root.destroy({ children: true });
    this.names.clear();
  }
}
