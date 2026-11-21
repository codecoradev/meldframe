// When the canvas shows all the content again by itself. The first fit happens as soon as there is content, but the boxes it fits
// to can still change: texts are measured with the stub until the page's fonts have loaded, and then the measurer is given again
// and the boxes grow or shrink. The fit is redone once for that, and only while it is the app's: the person has not zoomed or
// panned since it. Pure: the page keeps the facts and asks.
import type { Camera } from '@meldframe/web-canvas/camera';

export const sameCamera = (a: Camera, b: Camera): boolean => a.x === b.x && a.y === b.y && a.zoom === b.zoom;

export interface RefitFacts {
  /** The first fit has been done. */
  fitted: boolean;
  /** The fit has been redone once already. */
  refitted: boolean;
  /** How many times the host had given the engine a measurer when the first fit was done, and now. */
  setsAtFit: number;
  sets: number;
  /** The camera the fit gave (null before it), and the camera now. */
  fitCamera: Camera | null;
  camera: Camera;
}

/** Whether to fit again: the measurer was given again since the first fit (the fonts settled), once, with the camera where the fit left it. */
export function shouldRefit(f: RefitFacts): boolean {
  return f.fitted && !f.refitted && f.sets > f.setsAtFit && f.fitCamera !== null && sameCamera(f.fitCamera, f.camera);
}
