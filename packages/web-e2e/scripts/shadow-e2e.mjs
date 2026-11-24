// Browser check of the shadows the canvas draws (issue 236): pixels of a real Chromium canvas at hard shadows (blur 0: offset, spread,
// a negative spread, the order of two, a radius, a circle, under a clip and under an opacity: each probe within 4/255 of the hand-computed colour), a blurred one against the
// blur CSS draws for the same box (close, not identical: the contract, section 8.5, allows 20/255), the same at other zooms of the
// camera (the strength of the filter follows the zoom), and the same pixels in a capture at scale 1 and 2, which takes the reach of
// the shadow in.
//   node packages/web-e2e/scripts/shadow-e2e.mjs <web-url> <server-url> <token>
// Start first: the server (`MELDFRAME_DEV_TOKENS="<token>:ws_dev:agent:dev" MELDFRAME_LISTEN=127.0.0.1:<port> meldframe-server`, in memory), the
// web client on a file that is new for each run (`MELDFRAME_SERVER=<server-url> MELDFRAME_WEB_TOKEN=<token> MELDFRAME_WEB_FILE=<file> npx vite
// --port <port> --strictPort` in apps/web, with the WASM bridge built from this revision), and the same file name in MELDFRAME_WEB_FILE here.
// The MCP session binds to the first file it captures, so a server that has captured another file needs a restart.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const { chromium } = createRequire(import.meta.url)('playwright');
const [web, server, token] = process.argv.slice(2);
const file = process.env.MELDFRAME_WEB_FILE;
if (!web || !server || !token || !file) throw new Error('usage: MELDFRAME_WEB_FILE=<new file> node shadow-e2e.mjs <web-url> <server-url> <token>');

const box = (id, type, parentId, x, y, w, h, more = {}) => ({ id, type, parentId, x, y, w, h, fill: '#cccccc', ...more });
const shadow = (x, y, blur, spread, color) => ({ x, y, blur, spread, color });
const nodes = [
  box('page', 'frame', undefined, 0, 0, 900, 480, { fill: '#ffffff', name: 'page' }),
  // Row 1: offset, spread, negative spread, order.
  box('h1', 'rect', 'page', 40, 40, 100, 60, { shadows: [shadow(20, 15, 0, 0, '#ff0000')] }),
  box('h2', 'rect', 'page', 220, 40, 100, 60, { shadows: [shadow(0, 0, 0, 10, '#0000ff')] }),
  box('h3', 'rect', 'page', 400, 40, 100, 60, { shadows: [shadow(30, 30, 0, -10, '#00aa00')] }),
  box('h4', 'rect', 'page', 580, 40, 100, 60, { shadows: [shadow(10, 10, 0, 0, '#ff0000'), shadow(20, 20, 0, 0, '#0000ff')] }),
  // Row 2: a radius, the ratio of a radius to a big spread, a circle, a clip frame, an opacity.
  box('r1', 'rect', 'page', 40, 190, 100, 60, { radius: 20, shadows: [shadow(0, 0, 0, 10, '#ff0000')] }),
  box('r2', 'rect', 'page', 230, 200, 80, 50, { radius: 6, shadows: [shadow(0, 0, 0, 30, '#0000ff')] }),
  box('c1', 'ellipse', 'page', 400, 190, 100, 100, { shadows: [shadow(0, 0, 0, 10, '#0000ff')] }),
  box('k1', 'frame', 'page', 580, 190, 100, 60, { clip: true, shadows: [shadow(20, 20, 0, 0, '#ff0000')] }),
  box('k1k', 'rect', 'k1', -30, -30, 300, 200, { fill: '#ffff00' }),
  // A child of the clipping frame casts a shadow beyond the frame: the frame cuts it (an ancestor's clip cuts a shadow, the node's own does not).
  box('k1c', 'rect', 'k1', 60, 0, 30, 15, { fill: '#00aa00', shadows: [shadow(40, 0, 0, 0, '#ff00ff')] }),
  box('o1', 'rect', 'page', 760, 190, 100, 60, { opacity: 0.5, shadows: [shadow(20, 20, 0, 0, '#0000ff')] }),
  // Row 3: an alpha colour, and the blurred one (a magenta box, which gives the camera back to the probes).
  box('a1', 'rect', 'page', 40, 340, 100, 60, { shadows: [shadow(20, 0, 0, 0, '#0000ff80')] }),
  // A translucent fill over its own shadow: CSS clips a shadow out of the box, the canvas draws it under the box (the contract's row `shadows`).
  box('g1', 'rect', 'page', 500, 340, 100, 60, { fill: '#cccccc80', shadows: [shadow(20, 0, 0, 0, '#0000ff')] }),
  box('b1', 'rect', 'page', 280, 360, 120, 80, { fill: '#ff00ff', shadows: [shadow(0, 0, 16, 0, '#000000')] }),
];
const RED = [255, 0, 0], BLUE = [0, 0, 255], GREEN = [0, 170, 0], GREY = [204, 204, 204], WHITE = [255, 255, 255], YELLOW = [255, 255, 0];
// [what, world x, world y, expected rgb, tolerance (default 4)]
const probes = [
  ['offset: right of the box', 150, 70, RED], ['offset: below the box', 100, 108, RED], ['offset: beyond the shadow', 165, 70, WHITE],
  ['offset: left of where the shadow starts', 50, 108, WHITE], ['offset: the box is over its shadow', 100, 70, GREY], ['offset: below the shadow', 100, 118, WHITE],
  ['spread: left of the box', 215, 70, BLUE], ['spread: right of the box', 325, 70, BLUE], ['spread: above the box', 280, 35, BLUE], ['spread: below the box', 280, 105, BLUE],
  ['spread: a square corner stays square', 211, 31, BLUE], ['spread: beyond it', 205, 70, WHITE], ['spread: beyond it (right)', 335, 70, WHITE], ['spread: below it', 280, 115, WHITE],
  ['negative spread: right of the box', 510, 90, GREEN], ['negative spread: below the box', 480, 110, GREEN], ['negative spread: beyond it', 525, 90, WHITE],
  ['negative spread: left of where it starts', 435, 110, WHITE], ['negative spread: below it', 470, 125, WHITE], ['negative spread: the box', 450, 90, GREY],
  ['order: where both shadows are, the second (the top) shows', 685, 90, BLUE], ['order: where only the first is', 685, 55, RED], ['order: where only the second is', 695, 115, BLUE],
  ['radius: the corner of the shadow is round', 31.5, 181.5, WHITE], ['radius: and round further in', 36, 186, WHITE], ['radius: inside the round corner', 42, 192, RED],
  // r = 6 and a spread of 30: the browser's ratio makes a radius of 20.6, not 36.
  ['radius under a big spread: the corner is as round as the browser draws it', 208, 178, BLUE], ['radius under a big spread: the shadow beyond the corner', 201, 171, WHITE],
  ['circle: above, inside the spread', 450, 185, BLUE], ['circle: above, beyond the spread', 450, 172, WHITE], ['circle: at 45 degrees, inside', 491, 281, BLUE], ['circle: at 45 degrees, beyond', 495, 285, WHITE],
  ['clip: the shadow is not cut by the frame (right)', 690, 230, RED], ['clip: the shadow is not cut by the frame (below)', 640, 260, RED],
  ['clip: a shadow of a child is cut by the clipping frame', 695, 197, WHITE], ['clip: the child of the frame', 655, 197, GREEN],
  ['clip: the child inside', 620, 210, YELLOW], ['clip: the child is cut at the edge', 575, 220, WHITE],
  ['opacity: the shadow is faded with the node', 870, 230, [127, 127, 255]],
  ['alpha: the colour of the shadow carries its own alpha', 150, 370, [127, 127, 255]],
  ['translucent fill: the shadow beyond the box', 610, 370, BLUE],
  // CSS draws no shadow under the border box, the canvas does: a box with a fill of 50% shows its shadow through it on the canvas (102,102,230: the
  // grey at half over blue) and not in the export (230,230,230: the grey at half over the page). The probes pin the canvas (tolerance 4), so a change of either side shows.
  ['translucent fill: the canvas shows the shadow through the box', 560, 370, [102, 102, 230], 4],
  // The same under an opacity of the node: Pixi fades each part alone (the box at half over the shadow at half over the page), CSS the node as one image with the shadow
  // hidden under the box (the grey at half over the page).
  ['opacity: the canvas shows the shadow through the box', 820, 230, [166, 166, 230], 4],
];

// The hard shadow that a capture of `h1` has to show: the node and the reach of its shadow, in its own pixels.
const capture = (scale) => ({ id: 'h1', scale, width: 120 * scale, height: 75 * scale, probes: [
  ['capture: the shadow right of the box', 110, 30, RED], ['capture: the shadow below the box', 60, 68, RED], ['capture: the box', 30, 20, GREY], ['capture: above the shadow, right of the box', 118, 3, null], ['capture: below the box, left of the shadow', 3, 72, null],
].map(([w, x, y, c]) => [`${w} (scale ${scale})`, x * scale, y * scale, c]) });

if (!process.env.MELDFRAME_SEEDED) {
  const res = await fetch(`${server}/v1/files/${file}/tools/apply_commands`, {
    method: 'POST', headers: { 'content-type': 'application/json', authorization: `Bearer ${token}` },
    body: JSON.stringify({ ops: nodes.map((n, i) => ({ key: `shadow-${i}`, command: { type: 'addNode', node: n }, deps: [] })) }),
  });
  assert.equal(res.status, 200, `apply_commands: ${res.status} ${await res.clone().text()}`);
}

// The contract (section 8.5) allows 20/255 between the canvas blur and CSS. On a 2x screen the filter keeps a resolution of 1 and the measured
// maximum is 25; the limit of 28 leaves headroom and has to be measured again when Chromium changes (the contract says so too).
const dpr = Number(process.env.MELDFRAME_DPR ?? 1);
const BLUR_MAX = dpr === 1 ? 20 : 28;
let failed = 0, checks = 0;
const check = (ok, line) => { checks++; if (!ok) failed++; console.log(`${ok ? 'ok  ' : 'FAIL'} ${line}`); };
const near = (px, want, tol = 40) => px.every((v, k) => Math.abs(v - want[k]) <= tol);

/** Runs in the page: a PNG drawn over white (a capture is transparent where nothing is drawn) into a canvas that `window.__im` keeps. */
const load = async (b64) => {
  const img = new Image(); img.src = 'data:image/png;base64,' + b64; await img.decode();
  const c = document.createElement('canvas'); c.width = img.width; c.height = img.height;
  const g = c.getContext('2d', { willReadFrequently: true }); g.fillStyle = '#fff'; g.fillRect(0, 0, c.width, c.height); g.drawImage(img, 0, 0);
  window.__im = { w: c.width, h: c.height, d: g.getImageData(0, 0, c.width, c.height).data };
  return { w: c.width, h: c.height };
};
/** Runs in the page, on `window.__im`: the red channel along a row, the pixels at points, and the box of the pixels of one colour. */
const sample = ({ points, row, box }) => {
  const im = window.__im;
  const at = (x, y) => { const i = (Math.round(y) * im.w + Math.round(x)) * 4; return [im.d[i], im.d[i + 1], im.d[i + 2]]; };
  let found = null;
  if (box) {
    let x0 = 1e9, y0 = 1e9, x1 = 0, y1 = 0;
    const [r, g, b] = box.rgb;
    for (let y = box.y0 ?? 0; y < (box.y1 ?? im.h); y++) for (let x = box.x0 ?? 0; x < (box.x1 ?? im.w); x++) {
      const i = (y * im.w + x) * 4;
      if (im.d[i] === r && im.d[i + 1] === g && im.d[i + 2] === b) { x0 = Math.min(x0, x); y0 = Math.min(y0, y); x1 = Math.max(x1, x + 1); y1 = Math.max(y1, y + 1); }
    }
    found = x1 > x0 ? { x0, y0, x1, y1 } : null;
  }
  return {
    w: im.w, h: im.h, found,
    points: (points ?? []).map(([x, y]) => at(x, y)),
    row: row ? Array.from({ length: row.n }, (_, i) => at(row.x - i * row.dx, row.y)[0]) : null,
  };
};

const browser = await chromium.launch({ args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
try {
  const page = await browser.newPage({ viewport: { width: 1440, height: 860 }, deviceScaleFactor: dpr });
  page.on('pageerror', (e) => { failed++; console.log('[pageerror]', e.message); });
  /** Loads a PNG (base64) into `page`, then reads what `q` asks of it. */
  const look = async (pg, b64, q) => {
    await pg.evaluate(`(${load})(${JSON.stringify(b64)})`);
    return pg.evaluate(`(${sample})(${JSON.stringify(q)})`);
  };
  const shotOf = async (pg) => (await pg.screenshot()).toString('base64');
  /** The CSS blur of the same box at the same scale, in the same browser: the falloff to the left of it, `n` pixels. */
  const cssProfile = async (k, blur, w, h, n) => {
    const ref = await browser.newPage({ viewport: { width: 900, height: 700 }, deviceScaleFactor: dpr });
    await ref.setContent(`<body style="margin:0;background:#fff"><div style="position:absolute;left:200px;top:200px;width:${w * k}px;height:${h * k}px;background:#ff00ff;box-shadow:0 0 ${blur * k}px 0 #000"></div>`);
    const r = await look(ref, await shotOf(ref), { row: { x: (200 - 1) * dpr, y: (200 + (h * k) / 2) * dpr, dx: 1, n } });
    await ref.close();
    return r.row;
  };
  await page.goto(web);
  await page.waitForSelector('[data-testid=connection][data-state=live]', { timeout: 60_000 });
  await page.waitForTimeout(3000);
  // The page frame is the one pure white area of the canvas: it gives the zoom and the origin, so the probes do not depend on the camera.
  const first = await look(page, await shotOf(page), { box: { rgb: WHITE, x0: 250, x1: 1440 * dpr - 250, y0: 100, y1: 860 * dpr - 60 } });
  assert.ok(first.found, 'the page frame was not found on the canvas');
  const { x0, y0, x1 } = first.found;
  const s = (x1 - x0) / 900;
  assert.ok(s > 0.3 * dpr && s < 3 * dpr, `the page frame was not found on the canvas (scale ${s})`);
  console.log(`camera zoom ${(s / dpr).toFixed(3)} at ${dpr}x`);
  const got = await page.evaluate(`(${sample})(${JSON.stringify({ points: probes.map(([, wx, wy]) => [x0 + wx * s, y0 + wy * s]) })})`);
  probes.forEach(([what, wx, wy, want, tol], i) => check(near(got.points[i], want, tol ?? 4), `${what} (${wx}, ${wy}): ${got.points[i].join(',')} want ${want.join(',')}`));

  // Culling: the canvas draws the view and half a view around it, and the culling reads the bounds of a node, which leave out the padding of a
  // filter. A node just beyond that margin whose blur halo reaches into the view must still draw its halo: a huge white shadow (a half plane
  // of blur 1200) whose box starts 8 px beyond the margin to the right of the page, read at the right edge of the view, 0.5 view + 11 px away.
  const [W, H, left, top] = await page.evaluate(() => {
    const c = [...document.querySelectorAll('canvas')].sort((a, b) => b.clientWidth * b.clientHeight - a.clientWidth * a.clientHeight)[0];
    const r = c.getBoundingClientRect();
    return [c.clientWidth, c.clientHeight, r.left, r.top];
  });
  const at = [(left + W - 3) * dpr, (top + 300) * dpr];
  const before = (await look(page, await shotOf(page), { points: [at] })).points[0];
  const zoomCss = s / dpr;
  const apply = (ops) => fetch(`${server}/v1/files/${file}/tools/apply_commands`, {
    method: 'POST', headers: { 'content-type': 'application/json', authorization: `Bearer ${token}` }, body: JSON.stringify({ ops }),
  });
  const halo = box('halo', 'rect', undefined, (left + 1.5 * W + 8 - x0 / dpr) / zoomCss, (top + 300 - y0 / dpr) / zoomCss - 10000, 20000, 20000, { fill: '#00ffff', shadows: [shadow(0, 0, 1200, 0, '#ffffff')] });
  assert.equal((await apply([{ key: 'halo-add', command: { type: 'addNode', node: halo }, deps: [] }])).status, 200);
  await page.waitForTimeout(2500);
  const after = (await look(page, await shotOf(page), { points: [at] })).points[0];
  check(after[0] - before[0] >= 6, `culling: the halo of a node beyond the margin of the view is drawn (the view's right edge, ${W} x ${H}: ${before.join(',')} -> ${after.join(',')}; want at least 6 lighter)`);
  // The policy does not let a call delete: the node is shrunk and loses its shadow instead.
  const gone = await apply([{ key: 'halo-gone', command: { type: 'updateNode', id: 'halo', patch: { w: 1, h: 1, shadows: [] } }, deps: [] }]);
  assert.equal(gone.status, 200, await gone.clone().text());
  await page.waitForTimeout(1500);

  /** The blurred shadow of `b1` against CSS: the magenta box gives the camera, the falloff to its left is compared pixel by pixel. */
  const blurAgainstCss = async (label) => {
    const im = await look(page, await shotOf(page), { box: { rgb: [255, 0, 255] } });
    assert.ok(im.found, `${label}: the magenta box is not on the canvas`);
    const { x0: bx, y0: by, x1: bx1, y1: by1 } = im.found;
    const k = (bx1 - bx) / dpr / 120; // CSS px per world unit
    const n = Math.floor(14 * k * dpr);
    const want = await cssProfile(k, 16, 120, 80, n + 2);
    const have = (await page.evaluate(`(${sample})(${JSON.stringify({ row: { x: bx - 1, y: (by + by1) / 2, dx: 1, n: n + 2 } })})`)).row;
    // The box's own anti-aliased edge pixel is not the pure colour, so the first pixel of the canvas's row is the edge pixel and the pixel past
    // it is the first one of the CSS row (a shift of -1), and the edge lies anywhere in that pixel: compare at the best shift from -1.5 to -0.5.
    const lerp = (a, x) => { const i = Math.min(a.length - 1, Math.max(0, Math.floor(x))); const j = Math.min(a.length - 1, i + 1); return a[i] + (a[j] - a[i]) * (Math.min(a.length - 1, Math.max(0, x)) - i); };
    let max = Infinity, shift = 0;
    for (let t = -1.5; t <= -0.5; t += 0.25) {
      const m = Math.max(...Array.from({ length: n }, (_, i) => Math.abs(lerp(want, i + 1 + t) - have[i + 1])));
      if (m < max) { max = m; shift = t; }
    }
    max = Math.round(max);
    const dark = Math.min(...have);
    check(max <= BLUR_MAX && dark < 200, `${label}: the falloff of the blur is within ${max}/255 of CSS (zoom ${k.toFixed(3)}, shift ${shift}, darkest ${dark}); allowed ${BLUR_MAX}`);
    if (max > BLUR_MAX) console.log(`     canvas ${have.join(',')}\n     css    ${want.join(',')}`);
    return { k, bx, my: (by + by1) / 2 };
  };
  const fit = await blurAgainstCss('blur at the fit');

  // The camera: ctrl and wheel. The strength of the filter has to follow the zoom, so the falloff keeps its shape in the box's own units.
  // The pointer stays on the left edge of the magenta box while it zooms, which keeps the box there (and out of the panels) as the world grows
  // or shrinks around it, and then goes away from it: the outline of a hovered node is drawn over its edge and would be read as shadow.
  const zooms = [fit.k];
  await page.keyboard.down('Control');
  for (const [label, dy, times] of [['blur zoomed in', -150, 3], ['blur zoomed out', 150, 5]]) {
    await page.mouse.move(fit.bx / dpr, fit.my / dpr);
    for (let i = 0; i < times; i++) { await page.mouse.wheel(0, dy); await page.waitForTimeout(150); }
    await page.mouse.move(1100, 780);
    await page.waitForTimeout(800);
    zooms.push((await blurAgainstCss(label).catch((e) => { failed++; console.log('FAIL', label, e.message); return fit; })).k);
  }
  await page.keyboard.up('Control');
  check(zooms.length === 3 && Math.abs(zooms[1] - zooms[0]) > 0.2 && Math.abs(zooms[2] - zooms[0]) > 0.2, `the camera moved: zooms ${zooms.map((z) => z.toFixed(2)).join(', ')}`);

  // The capture the agent gets, through the MCP endpoint, from the editor that is open on the file.
  const shot = async (args) => {
    for (let i = 0; i < 20; i++) {
      const r = await fetch(`${server}/mcp`, {
        method: 'POST',
        headers: { 'content-type': 'application/json', accept: 'application/json, text/event-stream', authorization: `Bearer ${token}` },
        body: JSON.stringify({ jsonrpc: '2.0', id: i + 1, method: 'tools/call', params: { name: 'capture', arguments: { file, ...args } } }),
      });
      const part = (await r.json()).result?.content?.[0];
      if (part?.type === 'image') return part.data;
      await page.waitForTimeout(1000);
    }
    throw new Error('the editor answered no capture');
  };
  for (const c of [capture(1), capture(2)]) {
    const im = await look(page, await shot({ nodeId: c.id, scale: c.scale }), { points: c.probes.map(([, x, y]) => [x, y]) });
    check(im.w === c.width && im.h === c.height, `capture at ${c.scale}: the image is the node and the reach of its shadow, ${im.w} x ${im.h} (want ${c.width} x ${c.height})`);
    c.probes.forEach(([what, x, y, want], i) => check(near(im.points[i], want ?? WHITE, 4), `${what} (${x}, ${y}): ${im.points[i].join(',')} want ${(want ?? WHITE).join(',')}`));
  }
  // The blurred shadow in a capture: the node and 16 units of blur around it, compared with CSS at the same scale.
  // Not measured at 2x: the CSS reference page is rendered at the page's dpr, a capture is always one pixel to a unit, and the comparison would
  // need a reference page of its own at dpr 1. The capture is the same code at any dpr (`renderRegion` at resolution 1).
  for (const sc of dpr === 1 ? [1, 2] : []) {
    const n = 14 * sc;
    const im = await look(page, await shot({ nodeId: 'b1', scale: sc }), { row: { x: 16 * sc - 1, y: (16 + 40) * sc, dx: 1, n } });
    check(im.w === (120 + 32) * sc && im.h === (80 + 32) * sc, `blur capture at ${sc}: ${im.w} x ${im.h} (want ${(120 + 32) * sc} x ${(80 + 32) * sc})`);
    const ref = await cssProfile(sc / dpr, 16, 120, 80, n);
    const max = Math.max(...ref.map((v, i) => Math.abs(v - im.row[i])));
    check(max <= BLUR_MAX && Math.min(...im.row) < 200, `blur capture at ${sc}: the falloff is within ${max}/255 of CSS (allowed ${BLUR_MAX})`);
  }
} finally {
  await browser.close();
}
if (failed) { console.log(`${failed} of ${checks} checks FAIL`); process.exit(1); }
console.log(`${checks} checks pass`);
