// What a canvas full of blurred shadows costs (issue 250): frame time of the editor in a real Chromium, with N = 10, 100 and 500 cards in
// view at the fit, each with one shadow (blur 24, offset 0 8, an alpha colour), against the same cards with a HARD shadow (blur 0) and with
// NO shadow. The cost of the blur is the difference to those two controls. Since issue 249 each shadow with a blur is a Pixi BlurFilter
// (quality 4) on its own Graphics, and none is cached, so every redrawn frame runs one filter per card.
//   node packages/web-e2e/scripts/shadow-perf.mjs
// By hand, from the repository root, with Playwright on the global module path (as the other scripts). Needs the server binary
// (`cargo build -p meldframe-server`; `MELDFRAME_SERVER_BIN` overrides `$CARGO_TARGET_DIR/debug/meldframe-server`), the WASM bridge built from this
// revision (`tools/build-wasm.sh`) and `npm ci` in apps/web. The script starts a fresh in-memory server and one Vite per N (the file the page opens is
// fixed when Vite starts, so each N has its own file, with a name that is new for each run), seeds the cards over `apply_commands`, switches the
// shadows of the cards with `updateNode` and stops everything at the end. `MELDFRAME_PERF_FRAMES=n` (default 90) sets the frames per measurement and
// `MELDFRAME_PERF_SIZES=10,100,500` the sizes.
// `MELDFRAME_GL_ARGS="<flags>"` replaces the GL flags of Chromium (the default is software rendering; an empty value takes the machine's own choice).
//
// READ THE NUMBERS WITH CARE. In a container the browser renders with a SOFTWARE rasteriser (SwiftShader): the output says which one it got (the
// WebGL renderer string). The absolute milliseconds then say nothing about a user's GPU, and must not be quoted as GPU timings. What they can show
// is the shape: how the cost grows with N, and the ratio against the controls.
//
// What is measured (the page is driven only by synthetic wheel events on the canvas, as a mouse or a trackpad does; nothing in the app is changed):
//   idle  a redraw of a camera that stays where it is (a pan of 0.02 px, back and forth, so that every frame is redrawn: the app draws on demand
//         and a still camera draws nothing at all)
//   pan   the camera moves 6 px a frame to the left for half of the frames and back for the other half
//   zoom  the camera zooms by about 8% in and out at the centre, one step each frame (the filters are retuned to the zoom on each one)
// Per frame, three times (median and 95th percentile, in ms, over the frames after 15 that are not counted):
//   frame  the interval from one animation frame to the next (what a user sees; it cannot be below a display refresh of 16.7 ms)
//   draw   the time from the start of the animation frame until the GPU has finished what the app drew in it (a `gl.finish()` after the app's
//          callback): it does not stop at the refresh, and it is where the software rasteriser shows. It is NOT a number for a GPU
//   cpu    the main-thread time of the app's own frame callbacks (the JavaScript of Pixi and the app, and the calls that hand the GL commands over;
//          `requestAnimationFrame` is wrapped to time them): this part does not depend on the rasteriser, but it was taken on this machine's CPU,
//          and the rasteriser's threads share it
// The `shaded px` column counts the pixels of a picture of the canvas (between the panels) that are neither the canvas, the page nor a card: it
// proves that the three variants of a scene differ (no shadow has the lowest count) and is no timing.
//
// The numbers of the container of issue 250 (software rasteriser) are in docs/fidelity-contract.md, section 7, after the bullet on the canvas shadow.
import { spawn } from 'node:child_process';
import { createRequire } from 'node:module';
import { createServer } from 'node:net';
import { randomBytes } from 'node:crypto';
import { fileURLToPath } from 'node:url';

const { chromium } = createRequire(import.meta.url)('playwright');
const root = fileURLToPath(new URL('../../../', import.meta.url));
const SIZES = (process.env.MELDFRAME_PERF_SIZES ?? '10,100,500').split(',').map(Number);
const FRAMES = Number(process.env.MELDFRAME_PERF_FRAMES ?? 90);
const WARM = 15;
const VARIANTS = ['none', 'hard', 'blur'];
const MODES = ['idle', 'pan', 'zoom'];
const GL_ARGS = process.env.MELDFRAME_GL_ARGS === undefined ? ['--use-gl=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] : process.env.MELDFRAME_GL_ARGS.split(' ').filter(Boolean);
const run = Math.random().toString(36).slice(2, 8);
const token = randomBytes(16).toString('hex');
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const freePort = () => new Promise((res, rej) => { const s = createServer().listen(0, '127.0.0.1', () => { const { port } = s.address(); s.close(() => res(port)); }).on('error', rej); });
const pct = (xs, p) => { const s = [...xs].sort((a, b) => a - b); return s[Math.min(s.length - 1, Math.floor((p / 100) * s.length))]; };
const f1 = (x) => x.toFixed(1);

const CARD = { w: 160, h: 100, gx: 40, gy: 40 };
const SHADOW = { x: 0, y: 8, blur: 24, spread: 0, color: '#00000066' };
const PAGE = '#ffffff', CARD_FILL = '#cccccc';
const shadowsOf = (variant) => (variant === 'none' ? [] : [{ ...SHADOW, blur: variant === 'hard' ? 0 : SHADOW.blur }]);
// A white page frame (without a frame the editor lays an empty-state card over the canvas) with the cards in a grid inside it.
const cards = (n) => {
  const cols = Math.ceil(Math.sqrt(n * 1.6)), rows = Math.ceil(n / cols), m = CARD.gx;
  const page = { id: 'page', type: 'frame', x: 0, y: 0, w: m + cols * (CARD.w + CARD.gx), h: m + rows * (CARD.h + CARD.gy), fill: PAGE, name: 'page' };
  const list = Array.from({ length: n }, (_, i) => ({
    id: `c${i}`, type: 'rect', parentId: 'page', x: m + (i % cols) * (CARD.w + CARD.gx), y: m + Math.floor(i / cols) * (CARD.h + CARD.gy), w: CARD.w, h: CARD.h,
    radius: 8, fill: CARD_FILL, name: `card ${i}`,
  }));
  return { page, list };
};

const children = [];
const start = (cmd, args, env, cwd) => {
  const p = spawn(cmd, args, { cwd, env: { ...process.env, ...env }, stdio: ['ignore', 'ignore', 'pipe'] });
  let err = '';
  p.stderr.on('data', (d) => { err += d; });
  p.exited = new Promise((r) => p.on('exit', (code) => r({ code, err })));
  children.push(p);
  return p;
};
const waitFor = async (url, what) => {
  for (let i = 0; i < 120; i++) { try { if ((await fetch(url)).ok) return; } catch { /* not up yet */ } await sleep(500); }
  throw new Error(`${what} did not come up at ${url}`);
};

// Runs in the page. Drives the camera with wheel events and returns the frame intervals and the draw times and the main-thread times of `frames` frames.
const drive = async ({ mode, frames, warm }) => {
  const cv = [...document.querySelectorAll('canvas')].sort((a, b) => b.clientWidth * b.clientHeight - a.clientWidth * a.clientHeight)[0];
  const gl = cv.getContext('webgl2') ?? cv.getContext('webgl');
  const r = cv.getBoundingClientRect();
  const cx = r.left + r.width / 2, cy = r.top + r.height / 2;
  const send = (dx, dy, ctrl) => cv.dispatchEvent(new WheelEvent('wheel', { bubbles: true, cancelable: true, clientX: cx, clientY: cy, deltaX: dx, deltaY: dy, ctrlKey: ctrl }));
  const total = frames + warm;
  const step = (i) => {
    if (mode === 'idle') send(i % 2 ? 0.02 : -0.02, 0, false);
    else if (mode === 'pan') send(i < total / 2 ? -6 : 6, 0, false);
    else send(0, i % 2 ? 8 : -8, true);
  };
  const tick = () => new Promise((res) => { const c = new MessageChannel(); c.port1.onmessage = () => res(); c.port2.postMessage(0); });
  // The main-thread time of the app's own frame callbacks (the JavaScript that draws, and the calls that hand the GL commands over): `rAF` is wrapped
  // for it, ours is not.
  const raf = window.requestAnimationFrame.bind(window);
  let busy = 0;
  window.requestAnimationFrame = (cb) => raf((t) => { const a = performance.now(); try { cb(t); } finally { busy += performance.now() - a; } });
  const frame = [], draw = [], cpu = [];
  let prev = 0;
  for (let i = 0; i < total; i++) {
    const t = await new Promise(raf);
    await tick(); // the app's own frame callback has run in this animation frame
    gl.finish();
    const done = performance.now();
    if (i >= warm && prev) { frame.push(t - prev); draw.push(done - t); cpu.push(busy); }
    busy = 0;
    prev = t;
    step(i);
  }
  window.requestAnimationFrame = raf;
  return { frame, draw, cpu };
};

// Runs in the page: the pixels of a picture of the canvas (the canvas between the two panels) that are neither the canvas, the page nor the card colour (the shadows, the edges of the cards, and the part of the chrome in that area, the same in each scene).
const shaded = async ({ b64, card, page }) => {
  const img = new Image(); img.src = 'data:image/png;base64,' + b64; await img.decode();
  const c = document.createElement('canvas'); c.width = img.width; c.height = img.height;
  const g = c.getContext('2d', { willReadFrequently: true }); g.drawImage(img, 0, 0);
  const d = g.getImageData(0, 0, c.width, c.height).data;
  const bg = [d[0], d[1], d[2]];
  let n = 0;
  for (let i = 0; i < d.length; i += 4) {
    const px = [d[i], d[i + 1], d[i + 2]];
    if (!px.every((v, k) => v === bg[k]) && !px.every((v, k) => v === card[k]) && !px.every((v, k) => v === page[k])) n++;
  }
  return n;
};

async function main() {
  const serverBin = process.env.MELDFRAME_SERVER_BIN ?? `${process.env.CARGO_TARGET_DIR ?? `${root}target`}/debug/meldframe-server`;
  const sp = await freePort();
  const server = `http://127.0.0.1:${sp}`;
  start(serverBin, [], { MELDFRAME_DEV_TOKENS: `${token}:ws_dev:agent:dev`, MELDFRAME_LISTEN: `127.0.0.1:${sp}` }, root);
  await waitFor(`${server}/v1/health`, 'the server');
  // A call takes at most 100 ops.
  const post = async (file, ops) => {
    for (let i = 0; i < ops.length; i += 100) {
      const res = await fetch(`${server}/v1/files/${file}/tools/apply_commands`, {
        method: 'POST', headers: { 'content-type': 'application/json', authorization: `Bearer ${token}` }, body: JSON.stringify({ ops: ops.slice(i, i + 100) }),
      });
      if (res.status !== 200) throw new Error(`apply_commands ${file}: ${res.status} ${await res.text()}`);
    }
  };
  const scenes = [];
  for (const n of SIZES) {
    const file = `shadowperf_${run}_${n}`;
    const wp = await freePort();
    start(`${root}apps/web/node_modules/.bin/vite`, ['--port', String(wp), '--strictPort', '--host', '127.0.0.1'], { MELDFRAME_SERVER: server, MELDFRAME_WEB_TOKEN: token, MELDFRAME_WEB_FILE: file }, `${root}apps/web`);
    scenes.push({ n, file, web: `http://127.0.0.1:${wp}` });
  }
  for (const s of scenes) await waitFor(s.web, `vite for ${s.n}`);

  const browser = await chromium.launch({ args: GL_ARGS });
  const viewport = { width: 1440, height: 900 }, dpr = 1;
  const rows = [];
  let info;
  try {
    for (const s of scenes) {
      const { page: pageNode, list: nodes } = cards(s.n);
      await post(s.file, [pageNode, ...nodes].map((nd, i) => ({ key: `add-${i}`, command: { type: 'addNode', node: nd.id === 'page' ? nd : { ...nd, shadows: shadowsOf('none') } }, deps: [] })));
      const page = await browser.newPage({ viewport, deviceScaleFactor: dpr });
      page.on('pageerror', (e) => console.error('[pageerror]', e.message));
      await page.goto(s.web);
      await page.waitForSelector('[data-testid=connection][data-state=live]', { timeout: 60_000 });
      await page.waitForTimeout(3000);
      info ??= await page.evaluate(() => {
        const cv = document.querySelector('canvas');
        const gl = cv.getContext('webgl2') ?? cv.getContext('webgl');
        const ext = gl?.getExtension('WEBGL_debug_renderer_info');
        return { renderer: gl && ext ? gl.getParameter(ext.UNMASKED_RENDERER_WEBGL) : 'unknown', vendor: gl && ext ? gl.getParameter(ext.UNMASKED_VENDOR_WEBGL) : 'unknown', webgl: gl ? gl.getParameter(gl.VERSION) : 'none' };
      });
      for (const variant of VARIANTS) {
        if (variant !== 'none') {
          await post(s.file, nodes.map((nd, i) => ({ key: `${variant}-${i}`, command: { type: 'updateNode', id: nd.id, patch: { shadows: shadowsOf(variant) } }, deps: [] })));
        }
        // Fit all (Shift+1) on a canvas that has the keyboard, and let the socket's update and the first draw come in.
        const box = await page.evaluate(() => { const r = [...document.querySelectorAll('canvas')].sort((a, b) => b.clientWidth * b.clientHeight - a.clientWidth * a.clientHeight)[0].getBoundingClientRect(); return { x: r.left, y: r.top, w: r.width, h: r.height }; });
        await page.mouse.click(box.x + 4, box.y + box.h / 2);
        await page.keyboard.press('Shift+1');
        await page.mouse.move(box.x + box.w / 2, box.y + 2);
        await page.waitForTimeout(s.n >= 500 && variant === 'blur' ? 6000 : 2500);
        const zoom = await page.locator('[data-testid=zoom]').first().innerText();
        const shot = (await page.screenshot({ clip: { x: 290, y: 10, width: 840, height: 810 } })).toString('base64');
        const px = await page.evaluate(`(${shaded})(${JSON.stringify({ b64: shot, page: [255, 255, 255], card: [204, 204, 204] })})`);
        const row = { n: s.n, variant, fit: zoom.trim(), shaded: px, m: {} };
        for (const mode of MODES) {
          const { frame, draw, cpu } = await page.evaluate(`(${drive})(${JSON.stringify({ mode, frames: FRAMES, warm: WARM })})`);
          row.m[mode] = { frame: [pct(frame, 50), pct(frame, 95)], draw: [pct(draw, 50), pct(draw, 95)], cpu: [pct(cpu, 50), pct(cpu, 95)], frames: frame.length };
          await page.waitForTimeout(1200); // the camera settles: the culling tightens and the filters are tuned
        }
        rows.push(row);
        console.error(`  done ${s.n} ${variant}`);
      }
      await page.close();
    }
    info.browser = `Chromium ${browser.version()}`;
    info.viewport = `${viewport.width}x${viewport.height}`;
    info.dpr = dpr;
  } finally {
    await browser.close();
  }
  return { info, rows };
}

try {
  const { info, rows } = await main();
  console.log(`shadow-perf ${new Date().toISOString()}`);
  console.log(`browser   ${info.browser}, headless, viewport ${info.viewport}, dpr ${info.dpr}`);
  console.log(`WebGL     ${info.webgl}; renderer "${info.renderer}" (vendor "${info.vendor}"); GL flags: ${GL_ARGS.join(' ') || '(none)'}`);
  console.log(`scene     cards ${CARD.w}x${CARD.h} in a grid, shadow ${JSON.stringify(SHADOW)} (hard: blur 0), ${FRAMES} frames per measurement after ${WARM} not counted`);
  if (/swiftshader|llvmpipe|software/i.test(`${info.renderer} ${info.vendor}`)) console.log('NOTE      a SOFTWARE rasteriser: the milliseconds say nothing about a GPU; read the shape and the ratios only');
  console.log('\n  ms, median / p95. frame = interval between animation frames, draw = start of the frame until the GL work is finished');
  console.log('   N  variant    fit  shaded px |      idle frame       idle draw |       pan frame        pan draw |      zoom frame       zoom draw');
  const cell = (c) => `${f1(c[0]).padStart(7)} /${f1(c[1]).padStart(7)}`;
  for (const r of rows) {
    console.log(`${String(r.n).padStart(4)}  ${r.variant.padEnd(7)} ${r.fit.padStart(6)} ${String(r.shaded).padStart(10)} | ${MODES.map((m) => `${cell(r.m[m].frame)}  ${cell(r.m[m].draw)}`).join(' | ')}`);
  }
  console.log('\n  cpu = main-thread ms of the app\'s frame callbacks (the JavaScript that draws and hands the GL commands over), median / p95');
  console.log('   N  variant |        idle cpu           pan cpu          zoom cpu');
  for (const r of rows) console.log(`${String(r.n).padStart(4)}  ${r.variant.padEnd(7)} | ${MODES.map((m) => cell(r.m[m].cpu)).join('  ')}`);
  console.log('\n  cost of the blur: blur minus hard (and minus none), median draw ms and the ratio blur / hard');
  console.log('   N |       idle                pan               zoom');
  for (const n of SIZES) {
    const g = (v) => rows.find((r) => r.n === n && r.variant === v);
    const cells = MODES.map((m) => {
      const b = g('blur').m[m].draw[0], h = g('hard').m[m].draw[0], z = g('none').m[m].draw[0];
      return `+${f1(b - h)} (x${(b / h).toFixed(1)}), +${f1(b - z)}`.padEnd(22);
    });
    console.log(`${String(n).padStart(4)} | ${cells.join(' ')}`);
  }
  console.log('\n  per card, blur minus hard, median draw ms: ' + SIZES.map((n) => `${n}: ${MODES.map((m) => (((rowsOf(rows, n, 'blur').m[m].draw[0] - rowsOf(rows, n, 'hard').m[m].draw[0]) / n)).toFixed(3)).join(' / ')}`).join('; ') + ' (idle / pan / zoom)');
} finally {
  for (const p of children) p.kill();
}
function rowsOf(rows, n, v) { return rows.find((r) => r.n === n && r.variant === v); }
