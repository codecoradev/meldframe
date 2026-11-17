// Editing the text of a text node: which node, what the key does, the action a commit makes and where the editor sits.
// Pure; the field itself is a view (`apps/web/src/TextEditor.svelte`).
import type { Action, Node } from '@meldframe/protocol';
import { rectToScreen, type Camera } from '@meldframe/web-canvas/camera';
import type { Rect } from '@meldframe/web-canvas/geometry';
import type { KeyLike } from './edit';

/** The node when it is a text node. */
export function textTarget(id: string, nodes: readonly Node[]): Node | null {
  const n = nodes.find((x) => x.id === id);
  return n && n.type === 'text' ? n : null;
}

/** Enter on exactly one selected text node starts editing it: the id, or null. */
export function editKeyTarget(k: KeyLike, selection: readonly string[], nodes: readonly Node[]): string | null {
  if (k.key !== 'Enter' || k.metaKey || k.ctrlKey || k.shiftKey || k.altKey) return null;
  if (selection.length !== 1) return null;
  return textTarget(selection[0]!, nodes) ? selection[0]! : null;
}

/** What committing the field does: one action for a changed text, nothing for an unchanged one. */
export function commitText(id: string, before: string, after: string): Action | null {
  return before === after ? null : { type: 'editText', id, text: after };
}

export interface TextBoxStyle {
  left: number;
  top: number;
  width: number;
  height: number;
  fontSize: number;
}

/** Where the field goes, in screen pixels: over the node, with the font scaled by the zoom and at least one line high. */
export function textBoxStyle(camera: Camera, world: Rect, fontSize: number): TextBoxStyle {
  const r = rectToScreen(camera, world);
  const font = fontSize * camera.zoom;
  return { left: r.x, top: r.y, width: Math.max(r.w, 10), height: Math.max(r.h, font), fontSize: font };
}
