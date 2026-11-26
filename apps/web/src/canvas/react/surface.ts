// The effect of the React canvas, as a function that a test can call without React. The surface is the canvas that Pixi draws on; the
// React copy makes one for each mount and takes it out with the cleanup, so that an effect that React runs twice in development
// (mount, cleanup, mount, on the same element) never hands Pixi a canvas that an earlier application used. Tried on 26 Nov 2026 with
// one canvas kept for both mounts (`/?react=canvas,strict`): the page logged "PixiJS Error: Could not retrieve shader source (WebGL
// context may be lost)" and "Could not initialize shader", probably because the first application was still starting when it was undone
// and destroyed its context after the second had taken the canvas (the cause is inferred, not traced). With a canvas for each mount
// the page logs no such error.
import type { CanvasDriver } from '../driver';

/** Mounts `driver` on `host` with a surface of its own, in front of the host's other children; the returned function undoes it. */
export function mountOn(driver: Pick<CanvasDriver, 'mount'>, host: HTMLElement, doc: Document = document): () => void {
  const surface = doc.createElement('canvas');
  surface.className = 'canvas-surface';
  host.prepend(surface);
  const stop = driver.mount(host, surface);
  return () => {
    stop();
    surface.remove();
  };
}
