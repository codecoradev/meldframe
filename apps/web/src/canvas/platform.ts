// The browser as the canvas driver uses it: the animation frame, the size and density of the host element, the page's fonts and
// reduced-motion setting, the Pixi application and the transport of the images of image fills. Nothing here decides anything;
// driver.ts holds the rules and takes this object, so a test gives it a fake.
import { Application } from 'pixi.js';
import { ASSET_TIMEOUT_MS, type HostWatchers, type Platform } from './api';

const density = (): number => window.devicePixelRatio || 1;

/** Watches the element and the page; the returned function stops it. */
function watch(host: HTMLElement, w: HostWatchers): () => void {
  let stopped = false;
  const observer = new ResizeObserver(() => w.resize());
  observer.observe(host);

  const motion = window.matchMedia?.('(prefers-reduced-motion: reduce)');
  const onMotion = () => w.reducedMotion(!!motion?.matches);
  onMotion();
  motion?.addEventListener('change', onMotion);

  // The media query names one density, so a change is seen once and the next one is watched anew.
  let ratio: MediaQueryList | undefined;
  const onDensity = () => {
    w.density(density());
    watchDensity();
  };
  function watchDensity(): void {
    ratio?.removeEventListener('change', onDensity);
    ratio = window.matchMedia?.(`(resolution: ${density()}dppx)`);
    ratio?.addEventListener('change', onDensity);
  }
  watchDensity();

  // A web font that arrives after the first draw: draw the text again. (`ready` can resolve before a face that the canvas has
  // not used yet starts loading, so also listen for `loadingdone`.)
  const onFonts = () => {
    if (!stopped) w.fonts();
  };
  void document.fonts?.ready.then(onFonts);
  document.fonts?.addEventListener('loadingdone', onFonts);

  return () => {
    stopped = true;
    document.fonts?.removeEventListener('loadingdone', onFonts);
    observer.disconnect();
    motion?.removeEventListener('change', onMotion);
    ratio?.removeEventListener('change', onDensity);
  };
}

export function browserPlatform(): Platform {
  return {
    requestFrame: (cb) => requestAnimationFrame(cb),
    cancelFrame: (id) => cancelAnimationFrame(id),
    dpr: density,
    measure: (host) => {
      const r = host.getBoundingClientRect();
      return { w: r.width, h: r.height };
    },
    watch,
    createApp: () => new Application(),
    assets: { fetch: (url) => fetch(url, { signal: AbortSignal.timeout(ASSET_TIMEOUT_MS) }), decode: (blob) => createImageBitmap(blob) },
  };
}
