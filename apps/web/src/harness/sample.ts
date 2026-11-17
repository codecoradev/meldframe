// Scenes for the development harness. The colours here are the colours of a sample document (what a design
// would contain), not UI colours, so they are plain numbers and not tokens.
import type { Scene, SceneNode } from '@meldframe/web-canvas/scene';

const FRAME = 0xe6ece8;
const BLOCK = 0x9aa8a0;
const DARK = 0x1b2420;
const MINT = 0xbfe3d2;
const TEXT_SIZE = 14;

/** The design's "Editor, panels closed" page: four frames drawn at 5%, so every size is 20 times the pixels there. */
const U = 20;

interface Spec {
  id: string;
  name: string;
  x: number;
  h: number;
  /** [x, y, w, h, fill] of each block in design pixels. */
  blocks: [number, number, number, number, number][];
}

const FRAMES: Spec[] = [
  {
    id: 'desktop', name: 'Landing Desktop', x: 520, h: 270,
    blocks: [[8, 8, 54, 22, MINT], [6, 60, 58, 14, DARK], [6, 84, 26, 22, BLOCK], [36, 84, 28, 22, BLOCK], [6, 118, 58, 10, BLOCK], [0, 168, 70, 36, DARK], [6, 214, 58, 8, BLOCK], [6, 228, 40, 8, BLOCK]],
  },
  {
    id: 'mobile', name: 'Landing Mobile', x: 600, h: 330,
    blocks: [[8, 8, 54, 22, MINT], [6, 60, 58, 14, DARK], [6, 84, 58, 22, BLOCK], [6, 118, 58, 10, BLOCK], [0, 190, 70, 40, DARK], [6, 250, 58, 8, BLOCK], [6, 264, 40, 8, BLOCK]],
  },
  {
    id: 'pricing', name: 'Pricing', x: 680, h: 190,
    blocks: [[8, 8, 54, 18, MINT], [6, 40, 58, 60, BLOCK], [6, 110, 58, 10, BLOCK], [0, 140, 70, 34, DARK]],
  },
  {
    id: 'security', name: 'Security', x: 760, h: 150,
    blocks: [[8, 8, 54, 18, MINT], [6, 40, 26, 40, BLOCK], [36, 40, 28, 40, BLOCK], [6, 96, 58, 10, BLOCK]],
  },
];

export const DESIGN_ZOOM = 1 / U;

export function designScene(): Scene {
  const nodes: SceneNode[] = [];
  for (const f of FRAMES) {
    nodes.push({ id: f.id, parent: null, kind: 'frame', x: f.x * U, y: 170 * U, w: 70 * U, h: f.h * U, fill: FRAME, radius: 20, name: f.name, clip: true });
    f.blocks.forEach(([x, y, w, h, fill], i) => {
      nodes.push({ id: `${f.id}-${i}`, parent: f.id, kind: 'rect', x: x * U, y: y * U, w: w * U, h: h * U, fill, radius: 20 });
    });
  }
  return { space: 'parent', nodes };
}

/** The id the design shows selected: the 62 x 10 block at the bottom of Security. */
export const SELECTED_ID = 'security-3';

/** `count` nodes in frames of 50 rectangles and a heading each, on a grid; for the performance measurement. */
export function manyScene(count: number): Scene {
  const nodes: SceneNode[] = [];
  const perFrame = 50;
  const frames = Math.ceil(count / (perFrame + 1));
  const cols = Math.ceil(Math.sqrt(frames));
  for (let f = 0; f < frames; f++) {
    const fx = (f % cols) * 520;
    const fy = Math.floor(f / cols) * 700;
    nodes.push({ id: `f${f}`, parent: null, kind: 'frame', x: fx, y: fy, w: 480, h: 660, fill: FRAME, radius: 8, name: `Frame ${f}`, clip: true });
    for (let i = 0; i < perFrame && nodes.length < count; i++) {
      const row = Math.floor(i / 5);
      const col = i % 5;
      // every tenth node is text, the rest rectangles
      if (i % 10 === 0) {
        nodes.push({ id: `f${f}-${i}`, parent: `f${f}`, kind: 'text', x: 12 + col * 90, y: 12 + row * 60, w: 80, h: 20, fill: DARK, text: `Item ${f}.${i}`, fontSize: TEXT_SIZE });
      } else {
        nodes.push({ id: `f${f}-${i}`, parent: `f${f}`, kind: 'rect', x: 12 + col * 90, y: 12 + row * 60, w: 80, h: 50, fill: i % 3 === 0 ? MINT : BLOCK, radius: i % 4 === 0 ? 6 : 0 });
      }
    }
  }
  return { space: 'parent', nodes };
}
