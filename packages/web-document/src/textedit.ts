// Editing the text of a text node: the key that starts it and where the field sits. Beginning, committing and cancelling are the
// app layer's (`AppState.textEdit`, `Action.textEdit`: decision 43, Q6); the field itself, with its keystrokes, its caret and the
// composition of an input method, is a view (`apps/web/src/TextEditor.svelte`). Pure.
import { rectToScreen, type Camera } from '@meldframe/web-canvas/camera';
import type { Rect } from '@meldframe/web-canvas/geometry';
import { textFieldStyle, type TextFieldStyle } from '@meldframe/web-canvas/textstyle';
import { isControl, type ControlTarget, type KeyLike } from './edit';

/**
 * Whether the key asks to edit the selected text: Enter with no modifier, not typed on a button, a link or another control
 * (it activates that control and must not also start an edit). Which node it edits, and whether it is a text node at all, is the
 * app layer's (`textEdit` with `beginSelected`); this is only the key, until the key input moves there (T7a).
 */
export function isEditKey(k: KeyLike, target: ControlTarget | null = null): boolean {
  return k.key === 'Enter' && !k.metaKey && !k.ctrlKey && !k.shiftKey && !k.altKey && !isControl(target);
}

export interface TextBoxStyle extends TextFieldStyle {
  left: number;
  top: number;
  width: number;
  height: number;
  fontSize: number;
}

/**
 * Where the field goes, in screen pixels: over the node, with the font scaled by the zoom and at least one line high, and the
 * weight and the alignment of the text as the canvas draws it (`text`: the drawn node, which the scene holds; nothing given
 * is regular and left).
 */
export function textBoxStyle(camera: Camera, world: Rect, fontSize: number, text: { fontWeight?: number; textAlign?: 'left' | 'center' | 'right' } = {}): TextBoxStyle {
  const r = rectToScreen(camera, world);
  const font = fontSize * camera.zoom;
  return { left: r.x, top: r.y, width: Math.max(r.w, 10), height: Math.max(r.h, font), fontSize: font, ...textFieldStyle(text) };
}
