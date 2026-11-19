// Frame-time measurement of the canvas with generated scenes: node packages/web-e2e/scripts/canvas-perf.mjs [base-url] [nodes]
// Needs the dev server (npm run dev) and Playwright. Headless Chromium on the software renderer (SwiftShader),
// so the numbers are an upper bound for a machine with a GPU and say nothing about one. Prints one JSON object.
import { createRequire } from 'node:module';

// Playwright is not a dependency of the package: it comes from the global module path.
const { chromium } = createRequire(import.meta.url)('playwright');

const base = process.argv[2] ?? 'http://localhost:5173';
const nodes = Number(process.argv[3] ?? 5000);
const GL_ARGS = ['--use-gl=angle', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'];

const pct = (xs, p) => {
  const s = [...xs].sort((a, b) => a - b);
  return +s[Math.min(s.length - 1, Math.floor((p / 100) * s.length))].toFixed(1);
};
const summary = (xs) => ({ frames: xs.length, p50: pct(xs, 50), p95: pct(xs, 95), max: +Math.max(...xs).toFixed(1) });
/** Frame intervals (rAF to rAF) and the main-thread time the canvas spent per frame by part. */
const report = ({ intervals, timing }) => ({
  interval: summary(intervals),
  overlayMs: summary(timing.map((t) => t.overlay)),
  renderCallMs: summary(timing.map((t) => t.render)),
  cullMs: summary(timing.filter((t) => t.cull > 0).map((t) => t.cull).concat([0])),
});

(async () => {
  const browser = await chromium.launch({ args: GL_ARGS });
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1 });
  page.on('pageerror', (e) => console.error('[pageerror]', e.message));
  await page.goto(`${base}/harness/canvas.html?scene=many&n=1`);
  await page.waitForSelector('[data-renderer]');
  await page.evaluate(() => document.fonts.ready);
  await page.waitForTimeout(500);

  const result = { nodes, viewport: '1440x900', renderer: (await page.evaluate(() => window.__harness.stats())).renderer };
  result.buildMs = +(await page.evaluate((n) => window.__harness.swapScene(n), nodes)).toFixed(0);
  result.nodesBuilt = (await page.evaluate(() => window.__harness.stats())).nodes;

  // incremental update: one node changed in a scene of `nodes`
  result.updateOneNodeMs = +(await page.evaluate(async () => {
    const s = window.__harness.scene();
    const t0 = performance.now();
    // the harness has no single-node setter; a drag does the same through the move intent path
    window.__harness.moveOne?.(s.nodes[2].id);
    await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
    return performance.now() - t0;
  })).toFixed(1);

  // fit everything, then pan across it and zoom in and out around the centre
  await page.evaluate(() => window.__harness.fit());
  await page.waitForTimeout(400);
  const fitted = await page.evaluate(() => window.__harness.camera());
  result.fitZoomPercent = Math.round(fitted.zoom * 100);
  result.drawnAtFit = (await page.evaluate(() => window.__harness.stats())).drawn;

  const pan = await page.evaluate(async () => window.__harness.animate(120, (i, c) => ({ ...c, x: c.x - 6, y: c.y - 3 })));
  result.panAtFit = report(pan);

  const zoom = await page.evaluate(async () => window.__harness.animate(120, (i, c) => {
    const f = i < 60 ? 1.03 : 1 / 1.03;
    const sx = 720, sy = 450;
    const z = c.zoom * f;
    return { x: sx - ((sx - c.x) / c.zoom) * z, y: sy - ((sy - c.y) / c.zoom) * z, zoom: z };
  }));
  result.zoomInOut = report(zoom);

  await page.evaluate(() => window.__harness.setCamera({ x: -2000, y: -2000, zoom: 1 }));
  await page.waitForTimeout(400);
  result.drawnAtZoom100 = (await page.evaluate(() => window.__harness.stats())).drawn;
  const pan100 = await page.evaluate(async () => window.__harness.animate(120, (i, c) => ({ ...c, x: c.x - 8, y: c.y - 4 })));
  result.panAt100 = report(pan100);

  console.log(JSON.stringify(result, null, 2));
  await browser.close();
})();
