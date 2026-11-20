import { describe, expect, it } from 'vitest';
import { worldToScreen, ZOOM_MAX, ZOOM_MIN, ZOOM_STEP, type Camera } from '@meldframe/web-canvas/camera';
import { zoomCamera, zoomItems, type ZoomCommand } from './zoom';

const camera: Camera = { x: 120, y: -40, zoom: 0.5 };
const centre = { x: 300, y: 200 };

describe('the zoom commands of the menu', () => {
  it('zoom in and out take one step of the keys, and 100% sets 1, all around the centre of the view, which stays where it is', () => {
    const before = worldToScreen(camera, centre.x, centre.y);
    for (const [cmd, zoom] of [['in', camera.zoom * ZOOM_STEP], ['out', camera.zoom / ZOOM_STEP], ['reset', 1]] as const) {
      const next = zoomCamera(cmd, camera, centre);
      const after = worldToScreen(next, centre.x, centre.y);
      expect(next.zoom, cmd).toBeCloseTo(zoom, 12);
      expect([after.x, after.y].map((v) => Math.round(v * 1e6))).toEqual([before.x, before.y].map((v) => Math.round(v * 1e6)));
    }
  });

  it('never leaves the zoom limits', () => {
    expect(zoomCamera('in', { ...camera, zoom: ZOOM_MAX }, centre).zoom).toBe(ZOOM_MAX);
    expect(zoomCamera('out', { ...camera, zoom: ZOOM_MIN }, centre).zoom).toBe(ZOOM_MIN);
  });
});

describe('the rows of the zoom menu', () => {
  const calls: ZoomCommand[] = [];
  const rows = (mac: boolean) => zoomItems({ mac, onZoom: (c) => calls.push(c) });

  it('has zoom in, zoom out, 100%, fit all and zoom to the selection, with test ids for the browser checks', () => {
    expect(rows(false).map((i) => [i.id, i.label, i.testId])).toEqual([
      ['in', 'Zoom in', 'zoom-in'], ['out', 'Zoom out', 'zoom-out'], ['reset', 'Zoom to 100%', 'zoom-reset'],
      ['fit', 'Fit all', 'zoom-fit'], ['fit-selection', 'Zoom to selection', 'zoom-fit-selection'],
    ]);
  });

  it('shows the keys the canvas answers to, written for the platform, none for 100%, and a row reports its command', () => {
    expect(rows(false).map((i) => i.shortcut)).toEqual(['+', '-', undefined, 'Shift+1', 'Shift+2']);
    expect(rows(true).map((i) => i.shortcut)).toEqual(['+', '-', undefined, '⇧1', '⇧2']);
    calls.length = 0;
    for (const r of rows(false)) r.onSelect();
    expect(calls).toEqual(['in', 'out', 'reset', 'fit', 'fit-selection']);
  });
});
