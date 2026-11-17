// Browser check of the canvas through the development harness: node scripts/canvas-e2e.mjs [base-url]
// Needs the dev server (npm run dev) and Playwright (resolved from the global module path). Not part of
// `npm test`: it needs a browser. Each check prints one line; a failure exits non-zero.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

// Playwright is not a dependency of the package: it comes from the global module path.
const { chromium } = createRequire(import.meta.url)('playwright');

const base = process.argv[2] ?? 'http://localhost:5173';
const GL_ARGS = ['--use-gl=angle', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'];
const results = [];
const check = async (name, fn) => {
  try {
    await fn();
    results.push(true);
    console.log(`ok   ${name}`);
  } catch (e) {
    results.push(false);
    console.log(`FAIL ${name}\n     ${e.message.split('\n').join('\n     ')}`);
  }
};

/** Pixel of the page at (x, y), read from a screenshot decoded in the page. */
async function pixel(page, x, y) {
  const b64 = (await page.screenshot()).toString('base64');
  return page.evaluate(async ({ b64, x, y }) => {
    const img = new Image();
    img.src = `data:image/png;base64,${b64}`;
    await img.decode();
    const c = document.createElement('canvas');
    c.width = img.width; c.height = img.height;
    const g = c.getContext('2d');
    g.drawImage(img, 0, 0);
    return Array.from(g.getImageData(x, y, 1, 1).data.slice(0, 3));
  }, { b64, x, y });
}

/** Uncaught page exceptions of every page; any of them fails the run. */
const pageErrors = [];
/** Every browser launched, so that a failed run closes them all. */
const browsers = [];
const launch = async (args) => {
  const b = await chromium.launch({ args });
  browsers.push(b);
  return b;
};

async function open(browser, query = '') {
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1 });
  page.on('pageerror', (e) => {
    pageErrors.push(e.message);
    console.log('[pageerror]', e.message);
  });
  await page.addInitScript(() => {
    // Track listeners that are added and not removed, to see whether a canvas leaves any behind.
    const live = new Map();
    window.__listeners = live;
    const add = EventTarget.prototype.addEventListener;
    const remove = EventTarget.prototype.removeEventListener;
    EventTarget.prototype.addEventListener = function (type, fn, opts) {
      const signal = opts && typeof opts === 'object' ? opts.signal : undefined;
      if (fn && !(signal && signal.aborted)) live.set(`${type}|${String(fn).length}|${live.size}|${Math.random()}`, { target: this, type, fn, signal });
      return add.call(this, type, fn, opts);
    };
    EventTarget.prototype.removeEventListener = function (type, fn, opts) {
      for (const [k, v] of live) if (v.target === this && v.type === type && v.fn === fn) live.delete(k);
      return remove.call(this, type, fn, opts);
    };
    window.__liveListeners = () => {
      const out = {};
      for (const v of live.values()) {
        if (v.signal && v.signal.aborted) continue;
        if (v.target instanceof Node && !v.target.isConnected) continue;
        const name = v.target === window ? 'window' : v.target === document ? 'document' : v.target.constructor.name;
        out[`${name}:${v.type}`] = (out[`${name}:${v.type}`] ?? 0) + 1;
      }
      return out;
    };
  });
  await page.goto(`${base}/harness/canvas.html${query}`);
  await page.waitForSelector('[data-renderer]');
  await page.evaluate(() => document.fonts.ready);
  await page.waitForTimeout(500);
  return page;
}

const H = (page, expr) => page.evaluate(`window.__harness.${expr}`);

(async () => {
  const browser = await launch(GL_ARGS);
  const page = await open(browser);

  await check('renders with WebGL and paints the frame fill and the canvas background', async () => {
    assert.equal((await H(page, 'stats()')).renderer, 'webgl');
    assert.deepEqual(await pixel(page, 523, 330), [230, 236, 232]);
    assert.deepEqual(await pixel(page, 100, 100), [22, 22, 24]);
  });

  // security-3 centre: (15900, 5420) world at 5% -> (795, 271)
  await check('a click selects the node under the pointer', async () => {
    await page.mouse.click(795, 271);
    assert.deepEqual(await H(page, 'selection()'), ['security-3']);
    await page.mouse.click(1200, 700);
    assert.deepEqual(await H(page, 'selection()'), []);
  });

  await check('a drag reports world deltas, a final intent on release, and the host applies them', async () => {
    await H(page, 'events.length = 0');
    await page.mouse.move(795, 271);
    await page.mouse.down();
    await page.mouse.move(815, 271, { steps: 4 });
    await page.mouse.up();
    const moves = (await H(page, 'events')).filter((e) => e.type === 'move');
    const last = moves.at(-1);
    assert.equal(last.final, true);
    assert.equal(last.cancelled, false);
    assert.equal(last.dx, 400); // 20 px at 5% = 400 world units
    assert.equal(moves.filter((m) => m.final).length, 1);
    const node = (await H(page, 'scene()')).nodes.find((n) => n.id === 'security-3');
    assert.equal(node.x, 120 + 400);
  });

  await check('Escape during a drag cancels it and the host puts the node back', async () => {
    await H(page, 'events.length = 0');
    await page.mouse.move(815, 271);
    await page.mouse.down();
    await page.mouse.move(850, 300, { steps: 3 });
    await page.keyboard.press('Escape');
    await page.mouse.up();
    const moves = (await H(page, 'events')).filter((e) => e.type === 'move');
    assert.equal(moves.at(-1).cancelled, true);
    const node = (await H(page, 'scene()')).nodes.find((n) => n.id === 'security-3');
    assert.equal(node.x, 520);
  });

  await check('a handle drag reports a resize intent', async () => {
    await H(page, 'events.length = 0');
    const n = (await H(page, 'scene()')).nodes.find((m) => m.id === 'security-3');
    // the selection is security-3 (it was selected by the drag); its south-east corner on screen
    const sx = (15200 + n.x + n.w) * 0.05;
    const sy = (3400 + n.y + n.h) * 0.05;
    await page.mouse.move(sx, sy);
    await page.mouse.down();
    await page.mouse.move(sx + 10, sy + 4, { steps: 3 });
    await page.mouse.up();
    const resizes = (await H(page, 'events')).filter((e) => e.type === 'resize');
    assert.equal(resizes.at(-1).handle, 'se');
    assert.equal(resizes.at(-1).final, true);
    assert.equal(resizes.at(-1).dx, 200);
  });

  await check('the wheel pans, ctrl+wheel zooms around the cursor and the zoom stays in limits', async () => {
    const c0 = await H(page, 'camera()');
    await page.mouse.move(700, 450);
    await page.mouse.wheel(0, 100);
    await page.waitForTimeout(200);
    const c1 = await H(page, 'camera()');
    assert.equal(c1.zoom, c0.zoom);
    assert.equal(c1.y, c0.y - 100);
    const before = { x: (700 - c1.x) / c1.zoom, y: (450 - c1.y) / c1.zoom };
    await page.keyboard.down('Control');
    await page.mouse.wheel(0, -40);
    await page.keyboard.up('Control');
    await page.waitForTimeout(200);
    const c2 = await H(page, 'camera()');
    assert.ok(c2.zoom > c1.zoom);
    assert.ok(Math.abs((700 - c2.x) / c2.zoom - before.x) < 1e-6 * Math.abs(before.x) + 1e-6);
    // Wheel input never leaves the zoom limits (ZOOM_MIN 0.02, ZOOM_MAX 64 in packages/web-canvas/src/camera.ts).
    await page.keyboard.down('Control');
    for (let i = 0; i < 60; i++) await page.mouse.wheel(0, 400);
    await page.waitForTimeout(200);
    const zoomedOut = (await H(page, 'camera()')).zoom;
    for (let i = 0; i < 120; i++) await page.mouse.wheel(0, -400);
    await page.waitForTimeout(200);
    const zoomedIn = (await H(page, 'camera()')).zoom;
    await page.keyboard.up('Control');
    assert.ok(zoomedOut < c2.zoom, `zooming out moved the zoom (${zoomedOut})`);
    assert.ok(zoomedOut >= 0.02 - 1e-9, `zoom stays at or above the minimum (${zoomedOut})`);
    assert.ok(zoomedIn > zoomedOut, `zooming in moved the zoom (${zoomedIn})`);
    assert.ok(zoomedIn <= 64 + 1e-9, `zoom stays at or below the maximum (${zoomedIn})`);
    await H(page, 'setCamera({ x: 0, y: 0, zoom: 0.05 })');
  });

  await check('two touch pointers pinch the camera and no node moves', async () => {
    await H(page, 'setCamera({ x: 0, y: 0, zoom: 0.05 })');
    await H(page, 'events.length = 0');
    const z0 = (await H(page, 'camera()')).zoom;
    await page.evaluate(() => {
      const host = document.querySelector('[data-renderer]');
      const ev = (type, id, x, y) => host.dispatchEvent(new PointerEvent(type, { pointerId: id, pointerType: 'touch', clientX: x, clientY: y, button: 0, bubbles: true, isPrimary: id === 11 }));
      ev('pointerdown', 11, 600, 450);
      ev('pointerdown', 12, 800, 450);
      for (let i = 1; i <= 5; i++) {
        ev('pointermove', 11, 600 - i * 20, 450);
        ev('pointermove', 12, 800 + i * 20, 450);
      }
      ev('pointerup', 12, 900, 450);
      ev('pointerup', 11, 500, 450);
    });
    await page.waitForTimeout(100);
    const z1 = (await H(page, 'camera()')).zoom;
    assert.ok(Math.abs(z1 / z0 - 2) < 1e-6, `zoom ratio ${z1 / z0}`);
    assert.equal((await H(page, 'events')).filter((e) => e.type === 'move').length, 0);
  });

  await check('mounting and unmounting the canvas three times leaves no listener behind', async () => {
    await H(page, 'setCamera({ x: 0, y: 0, zoom: 0.05 })');
    const base = await page.evaluate(() => window.__liveListeners());
    await H(page, 'remount(3)');
    await page.waitForSelector('[data-renderer]');
    await page.waitForTimeout(300);
    const after = await page.evaluate(() => window.__liveListeners());
    assert.deepEqual(after, base, `before ${JSON.stringify(base)}\nafter  ${JSON.stringify(after)}`);
  });

  await check('hover reports the node under the pointer and clears over empty canvas', async () => {
    await H(page, 'events.length = 0');
    await page.mouse.move(535, 300);
    await page.mouse.move(1300, 800);
    const hovers = (await H(page, 'events')).filter((e) => e.type === 'hover').map((e) => e.id);
    assert.ok(hovers.length >= 2);
    assert.equal(hovers.at(-1), null);
  });
  await page.close();

  const dense = await browser.newPage({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });
  await check('devicePixelRatio 2: the canvas backing store is twice the CSS size', async () => {
    await dense.goto(`${base}/harness/canvas.html`);
    await dense.waitForSelector('[data-renderer]');
    const s = await dense.evaluate(() => {
      const c = document.querySelector('[data-renderer] canvas');
      return { w: c.width, h: c.height, cw: c.clientWidth, dpr: window.__harness.stats().dpr };
    });
    assert.equal(s.dpr, 2);
    assert.equal(s.w, s.cw * 2);
  });
  await dense.close();
  await browser.close();

  // WebGL switched off: Pixi's 2D canvas renderer takes over.
  const soft = await launch(['--disable-3d-apis', '--disable-gpu']);
  const p2 = await open(soft);
  await check('without WebGL it falls back to the canvas renderer and still paints', async () => {
    assert.equal((await H(p2, 'stats()')).renderer, 'canvas');
    assert.deepEqual(await pixel(p2, 523, 330), [230, 236, 232]);
  });
  await soft.close();

  const failed = results.filter((r) => !r).length + pageErrors.length;
  if (pageErrors.length) console.log(`${pageErrors.length} uncaught page error(s)`);
  console.log(failed ? `${failed} of ${results.length} checks failed` : `all ${results.length} checks passed`);
  process.exit(failed ? 1 : 0);
})().catch(async (e) => {
  console.error(`canvas-e2e: ${e.message} (is the dev server running? pass its URL as the first argument)`);
  await Promise.allSettled(browsers.map((b) => b.close()));
  process.exit(1);
});
