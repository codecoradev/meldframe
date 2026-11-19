// The DOM events of the canvas host turned into calls on the interaction. Plain structural types (no DOM
// lib types, no Pixi) so a test drives it with an EventTarget; the component owns the drawing state and passes
// in what the events need to touch (`InputHooks`).
import { screenToWorld, type Camera } from '@meldframe/web-canvas/camera';
import type { Interaction, Mods, ToolMode } from '@meldframe/web-canvas/interaction';

/** What the handlers read from a pointer, wheel, key or focus event. */
interface PointerLike {
  pointerId: number;
  clientX: number;
  clientY: number;
  button: number;
  pointerType: string;
}
interface WheelLike {
  clientX: number;
  clientY: number;
  deltaX: number;
  deltaY: number;
  deltaMode: number;
  ctrlKey: boolean;
  metaKey: boolean;
}
interface KeyLike {
  key: string;
  code: string;
}
interface ModsLike {
  shiftKey: boolean;
  altKey: boolean;
  ctrlKey: boolean;
  metaKey: boolean;
}

export interface InputEvents {
  pointerdown: PointerLike & ModsLike;
  pointermove: PointerLike & ModsLike;
  pointerup: PointerLike & ModsLike;
  pointercancel: object;
  lostpointercapture: object;
  pointerleave: object;
  wheel: WheelLike & { preventDefault(): void };
  keydown: KeyLike & ModsLike & { preventDefault(): void };
  keyup: KeyLike & ModsLike & { preventDefault(): void };
  blur: object;
  contextmenu: { preventDefault(): void };
  dblclick: { clientX: number; clientY: number };
}

/** The element the canvas lives in. */
export interface InputHost {
  addEventListener: EventTarget['addEventListener'];
  getBoundingClientRect(): { left: number; top: number };
  focus(opts?: { preventScroll: boolean }): void;
  setPointerCapture(id: number): void;
  style: { cursor: string };
}

export interface InputHooks {
  interaction: Pick<Interaction, 'down' | 'move' | 'up' | 'cancel' | 'leave' | 'wheel' | 'key' | 'cursor'>;
  tool(): ToolMode;
  camera(): Camera;
  /** The id of the node at a world point. */
  hit(x: number, y: number): string | null;
  /** Ask for another frame. */
  schedule(): void;
  /** Forget the rubber-band rectangle being drawn. */
  dropDraft(): void;
  /** A wheel gesture moved the camera: settle (tighten the culling, report the final camera) soon. */
  settleSoon(): void;
  /** A double click on a node with the move tool. */
  activate(id: string): void;
}

export const modsOf = (e: ModsLike): Mods => ({ shift: e.shiftKey, alt: e.altKey, ctrl: e.ctrlKey, meta: e.metaKey });

/** Listens on `host`; the returned function removes every listener. */
export function attachInput(host: InputHost, hooks: InputHooks): () => void {
  const { interaction } = hooks;
  const ac = new AbortController();
  const on = <K extends keyof InputEvents>(type: K, fn: (e: InputEvents[K]) => void, opts?: { passive?: boolean }) =>
    host.addEventListener(type, fn as (e: Event) => void, { ...opts, signal: ac.signal });
  const local = (e: { clientX: number; clientY: number }) => {
    const r = host.getBoundingClientRect();
    return { x: e.clientX - r.left, y: e.clientY - r.top };
  };
  const pointer = (e: PointerLike & ModsLike) => ({
    id: e.pointerId, ...local(e), button: e.button, pointerType: e.pointerType, mods: modsOf(e),
  });
  const refreshCursor = () => {
    host.style.cursor = interaction.cursor;
  };
  const after = () => {
    refreshCursor();
    hooks.schedule();
  };

  on('pointerdown', (e) => {
    host.focus({ preventScroll: true });
    try {
      host.setPointerCapture(e.pointerId);
    } catch {
      // a synthetic or already-ended pointer cannot be captured: the gesture still works inside the element
    }
    interaction.down(pointer(e));
    after();
  });
  on('pointermove', (e) => {
    interaction.move(pointer(e));
    after();
  });
  on('pointerup', (e) => {
    interaction.up(pointer(e));
    after();
  });
  on('pointercancel', () => {
    interaction.cancel();
    hooks.dropDraft();
    after();
  });
  on('pointerleave', () => interaction.leave());
  on('dblclick', (e) => {
    if (hooks.tool() !== 'move') return;
    const p = local(e);
    const w = screenToWorld(hooks.camera(), p.x, p.y);
    const id = hooks.hit(w.x, w.y);
    if (id) hooks.activate(id);
  });
  on('wheel', (e) => {
    e.preventDefault();
    interaction.wheel({ ...local(e), deltaX: e.deltaX, deltaY: e.deltaY, deltaMode: e.deltaMode, ctrlKey: e.ctrlKey, metaKey: e.metaKey });
    hooks.settleSoon();
  }, { passive: false });
  const key = (down: boolean) => (e: KeyLike & ModsLike & { preventDefault(): void }) => {
    if (interaction.key({ key: e.key, code: e.code, down, mods: modsOf(e) })) e.preventDefault();
    if (interaction.cursor !== 'crosshair') hooks.dropDraft();
    after();
  };
  on('keydown', key(true));
  on('keyup', key(false));
  on('blur', () => {
    interaction.key({ key: ' ', down: false, mods: modsOf({ shiftKey: false, altKey: false, ctrlKey: false, metaKey: false }) });
    refreshCursor();
  });
  on('contextmenu', (e) => e.preventDefault());
  return () => ac.abort();
}
