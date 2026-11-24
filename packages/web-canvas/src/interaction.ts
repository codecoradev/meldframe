// Pointer, wheel and key handling of the canvas as a state machine over plain data. It reads what it needs
// through `Env` and reports through `Intents`; it applies nothing itself. Pure: no DOM, no Pixi, so a test
// drives it with numbers. Editor rules (what a move means, snapping, minimum sizes, how a click toggles a
// selection) belong to the host: this module only says what the user did, in world units.
import {
  ZOOM_STEP, applyWheel, panBy, pinch, zoomAt, type Camera, type Pair, type WheelIn,
} from './camera';
import type { Point, Rect, Size } from './geometry';
import { handleAt, resizeCursor, type HandleId } from './overlay';

export type ToolMode = 'move' | 'hand' | 'frame' | 'rect' | 'ellipse' | 'text';
export type CreateTool = Exclude<ToolMode, 'move' | 'hand'>;

/** Pointer travel (screen px) before a press becomes a drag. */
export const DRAG_THRESHOLD = 3;

export interface Mods {
  shift: boolean;
  alt: boolean;
  ctrl: boolean;
  meta: boolean;
}

export interface PointerIn {
  id: number;
  /** CSS px from the canvas's top-left corner. */
  x: number;
  y: number;
  button: number;
  pointerType: string;
  mods: Mods;
}

export interface KeyIn {
  key: string;
  code?: string;
  down: boolean;
  mods: Mods;
}

interface GestureBase {
  /** True on the last intent of a gesture (release, or cancel). */
  final: boolean;
  /** True when the gesture was abandoned: the host puts back what it showed. */
  cancelled: boolean;
  mods: Mods;
}

/**
 * `dx`, `dy` are the total pointer travel since the press, in world units. `at` is where the pointer is, in world units
 * (where it was pressed on a cancel): a drop is decided by the place, not only by the distance.
 */
export interface MoveIntent extends GestureBase {
  ids: string[];
  dx: number;
  dy: number;
  at: Point;
}

export interface ResizeIntent extends GestureBase {
  ids: string[];
  handle: HandleId;
  dx: number;
  dy: number;
}

/** A rubber-band rectangle in world units, normalised to a positive size; size 0 on a plain click. */
export interface CreateIntent extends GestureBase {
  tool: CreateTool;
  /** The node under the press, for the host to pick a parent. */
  at: string | null;
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface Intents {
  select(ids: string[], additive: boolean): void;
  move(i: MoveIntent): void;
  resize(i: ResizeIntent): void;
  create(i: CreateIntent): void;
  /** `final` is false while the camera is still moving (pan drag, wheel) and true when it settles. */
  camera(c: Camera, final: boolean): void;
  hover(id: string | null): void;
  fit(kind: 'all' | 'selection'): void;
}

export interface Env {
  camera(): Camera;
  selection(): readonly string[];
  tool(): ToolMode;
  viewport(): Size;
  /** The front-most node under a world point. */
  hit(wx: number, wy: number): string | null;
  /** The selection's box in screen px, or null. */
  selectionBox(): Rect | null;
}

type Mode =
  | { k: 'idle' }
  | { k: 'dead' } // a gesture ended early (cancel, leftover pinch finger): ignore until every pointer is up
  | { k: 'pan'; last: Point }
  | { k: 'press'; id: string | null; wasSelected: boolean; additive: boolean; start: Point; touch: boolean; last: Point }
  | { k: 'move'; ids: string[]; start: Point }
  | { k: 'resize'; ids: string[]; handle: HandleId; start: Point; began: boolean }
  | { k: 'create'; tool: CreateTool; at: string | null; start: Point; began: boolean }
  | { k: 'pinch' };

const dist = (a: Point, b: Point) => Math.hypot(a.x - b.x, a.y - b.y);

export class Interaction {
  private mode: Mode = { k: 'idle' };
  private touches = new Map<number, Point>();
  private space = false;
  private lastHover: string | null = null;
  private hoverHandle: HandleId | null = null;
  private mods: Mods = { shift: false, alt: false, ctrl: false, meta: false };

  constructor(private env: Env, private out: Intents) {}

  /** The CSS cursor for the current state. */
  get cursor(): string {
    const m = this.mode;
    if (m.k === 'pan') return 'grabbing';
    if (m.k === 'resize') return resizeCursor(m.handle);
    if (m.k === 'create') return 'crosshair';
    if (m.k === 'idle') {
      if (this.space || this.env.tool() === 'hand') return 'grab';
      const t = this.env.tool();
      if (t !== 'move') return 'crosshair';
      if (this.hoverHandle) return resizeCursor(this.hoverHandle);
    }
    return 'default';
  }

  private world(p: Point): Point {
    const c = this.env.camera();
    return { x: (p.x - c.x) / c.zoom, y: (p.y - c.y) / c.zoom };
  }

  private delta(from: Point, to: Point): Point {
    const z = this.env.camera().zoom;
    return { x: (to.x - from.x) / z, y: (to.y - from.y) / z };
  }

  down(e: PointerIn): void {
    this.mods = e.mods;
    if (e.pointerType === 'touch') {
      this.touches.set(e.id, { x: e.x, y: e.y });
      if (this.touches.size === 2) {
        this.abandon();
        this.mode = { k: 'pinch' };
        return;
      }
      if (this.touches.size > 2) return;
    }
    if (this.mode.k !== 'idle') return;
    const here = { x: e.x, y: e.y };
    const tool = this.env.tool();
    if (e.button === 1 || (e.button === 0 && (tool === 'hand' || this.space))) {
      this.mode = { k: 'pan', last: here };
      return;
    }
    if (e.button !== 0) return;
    if (tool === 'move') {
      const box = this.env.selectionBox();
      const handle = box ? handleAt(box, e.x, e.y) : null;
      if (handle) {
        this.mode = { k: 'resize', ids: [...this.env.selection()], handle, start: here, began: false };
        return;
      }
      const w = this.world(here);
      const id = this.env.hit(w.x, w.y);
      const sel = this.env.selection();
      const wasSelected = id !== null && sel.includes(id);
      if (id !== null && !wasSelected) this.out.select([id], e.mods.shift);
      this.mode = {
        k: 'press', id, wasSelected, additive: e.mods.shift, start: here, touch: e.pointerType === 'touch', last: here,
      };
      return;
    }
    if (tool === 'hand') return;
    const w = this.world(here);
    this.mode = { k: 'create', tool, at: this.env.hit(w.x, w.y), start: here, began: false };
  }

  move(e: PointerIn): void {
    this.mods = e.mods;
    const here = { x: e.x, y: e.y };
    const m = this.mode;
    if (e.pointerType === 'touch' && this.touches.has(e.id)) {
      const before = [...this.touches.values()];
      this.touches.set(e.id, here);
      if (m.k === 'pinch') {
        const after = [...this.touches.values()];
        if (before.length === 2 && after.length === 2) {
          this.out.camera(pinch(this.env.camera(), before as unknown as Pair, after as unknown as Pair), false);
        }
        return;
      }
    }
    switch (m.k) {
      case 'idle':
        this.hovering(here);
        return;
      case 'pan': {
        this.out.camera(panBy(this.env.camera(), here.x - m.last.x, here.y - m.last.y), false);
        m.last = here;
        return;
      }
      case 'press': {
        if (dist(here, m.start) < DRAG_THRESHOLD) return;
        if (m.id === null) {
          if (m.touch) this.mode = { k: 'pan', last: m.last };
          else return;
          this.move(e);
          return;
        }
        const sel = this.env.selection();
        const ids = m.wasSelected ? [...sel] : m.additive ? (sel.includes(m.id) ? [...sel] : [...sel, m.id]) : [m.id];
        this.mode = { k: 'move', ids, start: m.start };
        this.move(e);
        return;
      }
      case 'move': {
        const d = this.delta(m.start, here);
        this.out.move({ ids: m.ids, dx: d.x, dy: d.y, at: this.world(here), final: false, cancelled: false, mods: e.mods });
        return;
      }
      case 'resize': {
        if (!m.began && dist(here, m.start) < DRAG_THRESHOLD) return;
        m.began = true;
        const d = this.delta(m.start, here);
        this.out.resize({ ids: m.ids, handle: m.handle, dx: d.x, dy: d.y, final: false, cancelled: false, mods: e.mods });
        return;
      }
      case 'create': {
        if (!m.began && dist(here, m.start) < DRAG_THRESHOLD) return;
        m.began = true;
        this.out.create(this.rubber(m, here, false, false));
        return;
      }
      default:
    }
  }

  private rubber(m: { tool: CreateTool; at: string | null; start: Point }, to: Point, final: boolean, cancelled: boolean): CreateIntent {
    const a = this.world(m.start);
    const b = this.world(to);
    return {
      tool: m.tool, at: m.at,
      x: Math.min(a.x, b.x), y: Math.min(a.y, b.y), w: Math.abs(a.x - b.x), h: Math.abs(a.y - b.y),
      final, cancelled, mods: this.mods,
    };
  }

  up(e: PointerIn): void {
    this.mods = e.mods;
    const here = { x: e.x, y: e.y };
    if (e.pointerType === 'touch') {
      this.touches.delete(e.id);
      if (this.mode.k === 'pinch') {
        if (this.touches.size < 2) {
          this.out.camera(this.env.camera(), true);
          this.mode = this.touches.size === 0 ? { k: 'idle' } : { k: 'dead' };
        }
        return;
      }
    }
    const m = this.mode;
    switch (m.k) {
      case 'pan':
        this.out.camera(this.env.camera(), true);
        break;
      case 'press':
        if (m.id === null) {
          if (!m.additive) this.out.select([], false);
        } else if (m.wasSelected) {
          if (m.additive) this.out.select([m.id], true);
          else if (this.env.selection().length > 1) this.out.select([m.id], false);
        }
        break;
      case 'move': {
        const d = this.delta(m.start, here);
        this.out.move({ ids: m.ids, dx: d.x, dy: d.y, at: this.world(here), final: true, cancelled: false, mods: e.mods });
        break;
      }
      case 'resize': {
        if (!m.began && dist(here, m.start) < DRAG_THRESHOLD) break;
        const d = this.delta(m.start, here);
        this.out.resize({ ids: m.ids, handle: m.handle, dx: d.x, dy: d.y, final: true, cancelled: false, mods: e.mods });
        break;
      }
      case 'create':
        this.out.create(
          m.began
            ? this.rubber(m, here, true, false)
            : { ...this.rubber(m, m.start, true, false) },
        );
        break;
      default:
    }
    this.mode = this.touches.size > 0 && e.pointerType === 'touch' ? { k: 'dead' } : { k: 'idle' };
  }

  /** The system took the pointer, or the user pressed Escape: abandon the gesture and report it. */
  cancel(): void {
    this.abandon();
    this.touches.clear();
    this.space = false; // a blur loses the key-up of a held Space
    this.mode = { k: 'idle' };
  }

  /** Reports the end of a gesture in progress as cancelled (or settled, for the camera). */
  private abandon(): void {
    const m = this.mode;
    const mods = this.mods;
    if (m.k === 'move') this.out.move({ ids: m.ids, dx: 0, dy: 0, at: this.world(m.start), final: true, cancelled: true, mods });
    else if (m.k === 'resize') this.out.resize({ ids: m.ids, handle: m.handle, dx: 0, dy: 0, final: true, cancelled: true, mods });
    else if (m.k === 'create') this.out.create({ ...this.rubber(m, m.start, true, true) });
    else if (m.k === 'pan' || m.k === 'pinch') this.out.camera(this.env.camera(), true);
  }

  leave(): void {
    this.hoverHandle = null;
    if (this.lastHover !== null) {
      this.lastHover = null;
      this.out.hover(null);
    }
  }

  private hovering(p: Point): void {
    const box = this.env.selectionBox();
    this.hoverHandle = this.env.tool() === 'move' && box ? handleAt(box, p.x, p.y) : null;
    const w = this.world(p);
    const id = this.env.tool() === 'move' && !this.hoverHandle ? this.env.hit(w.x, w.y) : null;
    if (id !== this.lastHover) {
      this.lastHover = id;
      this.out.hover(id);
    }
  }

  wheel(e: WheelIn & { x: number; y: number }): void {
    this.out.camera(applyWheel(this.env.camera(), e, e.x, e.y, this.env.viewport()), false);
  }

  /** Returns true when the key was a canvas key, so the caller can stop the browser's own handling. */
  key(e: KeyIn): boolean {
    this.mods = e.mods;
    if (e.key === ' ' || e.code === 'Space') {
      this.space = e.down;
      return true;
    }
    if (!e.down) return false;
    if (e.key === 'Escape') {
      if (this.mode.k !== 'idle' && this.mode.k !== 'dead') {
        this.abandon();
        this.mode = { k: 'dead' };
        return true;
      }
      return false;
    }
    if (e.mods.ctrl || e.mods.meta || e.mods.alt) return false;
    const vp = this.env.viewport();
    if (e.key === '+' || e.key === '=') {
      this.out.camera(zoomAt(this.env.camera(), ZOOM_STEP, vp.w / 2, vp.h / 2), true);
      return true;
    }
    if (e.key === '-' || e.key === '_') {
      this.out.camera(zoomAt(this.env.camera(), 1 / ZOOM_STEP, vp.w / 2, vp.h / 2), true);
      return true;
    }
    if (e.mods.shift && (e.code === 'Digit1' || e.key === '1' || e.key === '!')) {
      this.out.fit('all');
      return true;
    }
    if (e.mods.shift && (e.code === 'Digit2' || e.key === '2' || e.key === '@')) {
      this.out.fit('selection');
      return true;
    }
    return false;
  }
}
