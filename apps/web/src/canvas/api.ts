// What a host and the canvas say to each other (E7.3, S11): the props the canvas draws from, the events it reports, and the
// browser as the driver uses it, the two time limits of the asset requests, and the one sentence the element shows when the start
// failed (both copies of the element take it from here). driver.ts holds the rules; Canvas.svelte and react/Canvas.tsx hand these over.
import type { Application } from 'pixi.js';
import type { Camera } from '@meldframe/web-canvas/camera';
import type { Rect, Size } from '@meldframe/web-canvas/geometry';
import type { CreateIntent, MoveIntent, ResizeIntent, ToolMode } from '@meldframe/web-canvas/interaction';
import type { Scene } from '@meldframe/web-canvas/scene';
import type { AssetDeps } from '@meldframe/web-assets/assets';
import type { Theme } from '@meldframe/web-tokens/tokens';
import type { Touch } from './overlayview';

/** The longest one asset request may take, and how often the failed ones are looked at (platform.ts and driver.ts read them). */
export const ASSET_TIMEOUT_MS = 15_000;
export const ASSET_RETRY_EVERY_MS = 5_000;

/** What the element says when no renderer could start (the banner over the canvas has the browser's message, made safe, beside it). */
export const CANVAS_FAILED = 'The canvas could not start in this browser.';

/** What the canvas draws from. The host gives all of it each time; unchanged values are told by identity (the camera by value). */
export interface CanvasProps {
  scene: Scene;
  /** Ids drawn as selected. */
  selection: readonly string[];
  camera: Camera;
  touches: readonly Touch[];
  /** Where a dragged node would land in a flow: a line in world units, or none. */
  indicator: Rect | null;
  tool: ToolMode;
  theme: Theme;
  /** Overrides the system's reduced-motion setting when set. */
  reducedMotion?: boolean;
}

/** What the canvas tells the host. Read when it is called, so the host may replace the object at any time. */
export interface CanvasEvents {
  /** `additive` is true with Shift held: the host decides whether that adds or toggles. */
  onSelect?: (ids: string[], additive: boolean) => void;
  onMove?: (i: MoveIntent) => void;
  onResize?: (i: ResizeIntent) => void;
  onCreate?: (i: CreateIntent) => void;
  /** `final` is false while the camera is still moving. */
  onCameraChange?: (camera: Camera, final: boolean) => void;
  onHover?: (id: string | null) => void;
  /** A double click on a node (with the select tool): the id of the node under the pointer. */
  onActivate?: (id: string) => void;
  /** The renderer that started. */
  onReady?: (info: { renderer: string }) => void;
  /** The error when no renderer could start. */
  onError?: (error: unknown) => void;
  /** An image of an image fill is missing or could not be loaded: its asset id and the stable error code. */
  onAssetProblem?: (id: string, code: string) => void;
  /** The renderer's name (or none) and whether the start failed: what the element shows. */
  onStatus?: (status: CanvasStatus) => void;
}

export interface CanvasStatus {
  renderer: string;
  failed: boolean;
}

/** What the host element reports about the page it is in. */
export interface HostWatchers {
  /** The element's size changed. */
  resize(): void;
  /** The system's reduced-motion setting, once at the start and on each change. */
  reducedMotion(on: boolean): void;
  /** The device pixel ratio changed. */
  density(dpr: number): void;
  /** A web font arrived: texts drawn with the stub's metrics are drawn again. */
  fonts(): void;
}

/** The browser, as far as the driver needs it (platform.ts has the real one); a test gives a fake. */
export interface Platform {
  requestFrame(cb: () => void): number;
  cancelFrame(id: number): void;
  dpr(): number;
  /** The size of the element in CSS pixels. */
  measure(host: HTMLElement): Size;
  /** Starts watching the element and the page; the returned function stops it. */
  watch(host: HTMLElement, w: HostWatchers): () => void;
  createApp(): Application;
  /** Where the images of image fills come from. */
  assets: AssetDeps;
}

/** Milliseconds spent in the last frame, by part, and the frames drawn; for measurements. */
export interface FrameTiming {
  cull: number;
  overlay: number;
  render: number;
  frames: number;
}
