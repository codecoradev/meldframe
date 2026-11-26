// The React copy of Canvas.svelte (E7.3, S11): the element, and the wiring to the driver (driver.ts), which is the same code. It
// mounts the existing Pixi canvas from a ref in an effect (docs/react-web-plan.md section 6; not @pixi/react: the scene comes from
// the app layer as data and a second reconciler over it would hold drawing rules twice, against U1). It holds no state of the
// document and applies no edit. The camera is a controlled prop and a callback: the canvas moves its own camera at once and
// reports it through `onCameraChange`; the host hands back the same camera or another one (see driver.ts). The styles are
// canvas.css, the file the Svelte copy imports too.
//
// Effects are idempotent, since React's strict mode runs each twice in development: the callbacks and the props are plain
// assignments, and the effect that mounts makes a surface of its own and undoes everything it made (surface.ts, driver.test.ts).
// `/?react=canvas,strict` draws it inside StrictMode (the island's `strict` option) to show it in a browser.
import { useCallback, useEffect, useImperativeHandle, useLayoutEffect, useRef, useState, type Ref, type RefObject } from 'react';
import { HOME } from '@meldframe/web-canvas/camera';
import type { Scene } from '@meldframe/web-canvas/scene';
import { CANVAS_FAILED, type CanvasEvents, type CanvasProps as Draws, type CanvasStatus } from '../api';
import { CanvasDriver, type CanvasHandle } from '../driver';
import { browserPlatform } from '../platform';
import { mountOn } from './surface';
import '../canvas.css';

/** What the canvas draws from, all of it (the props of the component with their defaults). */
export type DrawProps = Draws;

export interface CanvasProps extends Partial<Omit<Draws, 'scene'>>, Omit<CanvasEvents, 'onStatus'> {
  scene: Scene;
  /** Accessible name of the canvas. */
  label?: string;
  /** What the editor calls: fit, centre, focus, capture (React 19 hands `ref` to a function component as a prop). */
  ref?: Ref<CanvasHandle>;
}

// Defaults that keep their identity between renders: the driver tells a change of the selection or the touches by identity.
const NO_IDS: readonly string[] = [];
const NO_TOUCHES: Draws['touches'] = [];
const NOT_STARTED: CanvasStatus = { renderer: '', failed: false };

/** The methods of the Svelte canvas (the `export function`s of Canvas.svelte), handed to the driver. */
export const handleOf = (d: CanvasDriver): CanvasHandle => ({
  fitAll: () => d.fitAll(),
  fitSelection: () => d.fitSelection(),
  centre: () => d.centre(),
  focus: () => d.focus(),
  captureBox: (id) => d.captureBox(id),
  capture: (box, scale) => d.capture(box, scale),
  stats: () => d.stats(),
});

/** The host element: what Canvas.svelte draws, as the same classes, role and name. The surface is made by the effect, not drawn here. */
export function CanvasView({ label, renderer, failed, hostRef }: { label: string; renderer: string; failed: boolean; hostRef?: Ref<HTMLDivElement> }) {
  return (
    <div ref={hostRef} className="canvas-host" role="application" aria-label={label} tabIndex={0} data-renderer={renderer || undefined}>
      {failed ? <p className="canvas-failed">{CANVAS_FAILED}</p> : null}
    </div>
  );
}

/**
 * The effects of the canvas, the part of it that touches the driver (a hook of its own so that a test can run it with a fake driver).
 * Each is idempotent: the callbacks and the props are plain assignments, and the mount makes a surface of its own and undoes it.
 */
export function useCanvasDriver(driver: CanvasDriver, host: RefObject<HTMLElement | null>, draws: DrawProps, events: Omit<CanvasEvents, 'onStatus'>, onStatus: (s: CanvasStatus) => void): void {
  const { scene, selection, camera, touches, indicator, tool, theme, reducedMotion } = draws;
  // The callbacks of this render: the driver reads them when it calls them, so no render leaves an old one behind.
  useLayoutEffect(() => {
    driver.events = { ...events, onStatus };
  });
  useLayoutEffect(() => {
    driver.setProps({ scene, selection, camera, touches, indicator, tool, theme, reducedMotion });
  }, [driver, scene, selection, camera, touches, indicator, tool, theme, reducedMotion]);
  useEffect(() => mountOn(driver, host.current!), [driver, host]);
}

export function Canvas({ scene, selection = NO_IDS, camera = HOME, touches = NO_TOUCHES, indicator = null, tool = 'move', theme = 'dark', reducedMotion, label = 'Canvas', ref, ...events }: CanvasProps) {
  const host = useRef<HTMLDivElement>(null);
  const [driver] = useState(() => new CanvasDriver(browserPlatform()));
  const [status, setStatus] = useState(NOT_STARTED);
  const onStatus = useCallback((s: CanvasStatus) => setStatus(s), []);
  useImperativeHandle(ref, () => handleOf(driver), [driver]);

  useCanvasDriver(driver, host, { scene, selection, camera, touches, indicator, tool, theme, reducedMotion }, events, onStatus);

  return <CanvasView label={label} renderer={status.renderer} failed={status.failed} hostRef={host} />;
}
