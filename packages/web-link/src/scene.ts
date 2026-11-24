// The scene the canvas draws, read from the painted boxes that the Rust render step hands over (`meldframe-render`,
// through `WasmApp.sceneJson`). A change of shape only: the canvas computes nothing of its own, so no order, no
// colour and no radius is worked out here. The fixtures of the Rust crate (`tests/fixtures/scene.json`) fix the JSON.
import type { SceneBorder } from '@meldframe/web-canvas/border';
import type { SceneShadow } from '@meldframe/web-canvas/shadow';
import type { NodeKind, Scene, SceneImage, SceneNode, SceneTextAlign } from '@meldframe/web-canvas/scene';

/** One box as `meldframe_render::PaintedBox` serialises it, back to front. */
export interface PaintedBox {
  id: string;
  parent: string | null;
  paint: NodeKind;
  x: number;
  y: number;
  w: number;
  h: number;
  /** `0xRRGGBB`; null for no fill. */
  fill: number | null;
  /** The alpha of the fill's own colour, 0 to 1; 1 when there is no fill. */
  fillAlpha: number;
  radius: number;
  text: string;
  fontSize: number;
  /** 100 to 900; 0 on a box that is not a text. */
  fontWeight: number;
  textAlign: SceneTextAlign;
  name: string;
  clip: boolean;
  opacity: number;
  gradient?: { kind: 'linear' | 'radial'; angle: number; stops: [pos: number, color: string][] };
  image?: SceneImage;
  /** Absent for no border (and always on a text). */
  border?: SceneBorder;
  /** Absent for none (and always on a text), in the model's order. */
  shadows?: SceneShadow[];
}

function toSceneNode(b: PaintedBox): SceneNode {
  const { gradient: g, image: i, border, shadows } = b;
  return {
    id: b.id, parent: b.parent, kind: b.paint, x: b.x, y: b.y, w: b.w, h: b.h, fill: b.fill, fillAlpha: b.fillAlpha, radius: b.radius,
    text: b.text, fontSize: b.fontSize, fontWeight: b.fontWeight, textAlign: b.textAlign, name: b.name, clip: b.clip, opacity: b.opacity,
    ...(g ? { gradient: { kind: g.kind, angle: g.angle, stops: g.stops.map(([pos, color]) => ({ pos, color })) } } : {}),
    ...(i ? { image: { assetId: i.assetId, fit: i.fit } } : {}),
    ...(border ? { border: { width: border.width, color: border.color, alpha: border.alpha, position: border.position } } : {}),
    ...(shadows?.length ? { shadows: shadows.map((s) => ({ x: s.x, y: s.y, blur: s.blur, spread: s.spread, color: s.color, alpha: s.alpha })) } : {}),
  };
}

/** The boxes in the order they arrive (parents before children, back to front), as a scene in parent space. */
export function sceneFromBoxes(boxes: readonly PaintedBox[]): Scene {
  return { space: 'parent', nodes: boxes.map(toSceneNode) };
}
