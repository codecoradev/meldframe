// The zoom menu of the status strip: its rows, and what the three commands that only change the zoom do to the camera.
// Fitting needs the content, so the editor hands those two to the canvas. Pure.
import { worldToScreen, zoomAt, zoomTo, ZOOM_STEP, type Camera } from '@meldframe/web-canvas/camera';
import { shortcutLabels } from './shortcuts';

export type ZoomCommand = 'in' | 'out' | 'reset' | 'fit' | 'fit-selection';

/**
 * The camera after zoom in, zoom out or 100%, around `centre`, the world point at the centre of the view: it stays
 * where it is on the screen, as it does for the plus and minus keys of the canvas.
 */
export function zoomCamera(cmd: 'in' | 'out' | 'reset', camera: Camera, centre: { x: number; y: number }): Camera {
  const at = worldToScreen(camera, centre.x, centre.y);
  return cmd === 'reset' ? zoomTo(camera, 1, at.x, at.y) : zoomAt(camera, cmd === 'in' ? ZOOM_STEP : 1 / ZOOM_STEP, at.x, at.y);
}

export interface ZoomItem {
  id: ZoomCommand;
  label: string;
  onSelect: () => void;
  shortcut?: string;
  testId: string;
}

const rows: { id: ZoomCommand; label: string; testId: string; key?: string }[] = [
  { id: 'in', label: 'Zoom in', testId: 'zoom-in', key: 'zoom-in' },
  { id: 'out', label: 'Zoom out', testId: 'zoom-out', key: 'zoom-out' },
  { id: 'reset', label: 'Zoom to 100%', testId: 'zoom-reset' },
  { id: 'fit', label: 'Fit all', testId: 'zoom-fit', key: 'fit-all' },
  { id: 'fit-selection', label: 'Zoom to selection', testId: 'zoom-fit-selection', key: 'fit-selection' },
];

/** The rows of the menu; each shows the first key the sheet lists for it, and 100% has none. */
export function zoomItems(o: { mac: boolean; onZoom: (c: ZoomCommand) => void }): ZoomItem[] {
  return rows.map((r) => ({ id: r.id, label: r.label, testId: r.testId, shortcut: r.key ? shortcutLabels(r.key, o.mac)[0] : undefined, onSelect: () => o.onZoom(r.id) }));
}
