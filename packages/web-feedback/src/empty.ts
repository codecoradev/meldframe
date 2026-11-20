// When the canvas is empty. Pure.

/** True when the file holds a frame. A file without one gets the empty state, even if it has loose shapes or text. */
export function hasFrames(nodes: readonly { type: string }[]): boolean {
  return nodes.some((n) => n.type === 'frame');
}
