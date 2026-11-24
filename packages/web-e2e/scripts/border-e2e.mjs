// Browser check of the borders the canvas draws (issue 236): pixels of a real Chromium canvas at the bands of an inside, a centred
// and an outside border, on a rounded box and an ellipse, over a child, under a clip and under an opacity (an outside border is exact;
// an inside one differs from the export, see the probe), and the same pixels in a capture, which takes the outside band in.
//   node packages/web-e2e/scripts/border-e2e.mjs <web-url> <server-url> <token>
// Start first: the server (`MELDFRAME_DEV_TOKENS="<token>:ws_dev:agent:dev" MELDFRAME_LISTEN=127.0.0.1:<port> meldframe-server`, in memory), the
// web client on a file that is new for each run (`MELDFRAME_SERVER=<server-url> MELDFRAME_WEB_TOKEN=<token> MELDFRAME_WEB_FILE=<file> npx vite
// --port <port> --strictPort` in apps/web, with the WASM bridge built from this revision), and the same file name in MELDFRAME_WEB_FILE here.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const { chromium } = createRequire(import.meta.url)('playwright');
const [web, server, token] = process.argv.slice(2);
const file = process.env.MELDFRAME_WEB_FILE;
if (!web || !server || !token || !file) throw new Error('usage: MELDFRAME_WEB_FILE=<new file> node border-e2e.mjs <web-url> <server-url> <token>');

const box = (id, type, parentId, x, y, w, h, more = {}) => ({ id, type, parentId, x, y, w, h, fill: '#cccccc', ...more });
const border = (width, color, position) => ({ width, color, position });
const nodes = [
  box('page', 'frame', undefined, 0, 0, 800, 400, { fill: '#ffffff', name: 'page' }),
  box('f1', 'frame', 'page', 20, 20, 140, 100, { border: border(6, '#ff0000', 'inside') }),
  box('r1', 'rect', 'page', 190, 20, 140, 100, { border: border(6, '#0000ff', 'outside') }),
  box('e1', 'ellipse', 'page', 360, 20, 100, 100, { border: border(6, '#00aa00', 'center') }),
  box('c1', 'frame', 'page', 520, 20, 140, 100, { clip: true, border: border(6, '#ff0000', 'outside') }),
  box('c1k', 'rect', 'c1', -30, -30, 300, 200, { fill: '#ffff00' }),
  box('f2', 'frame', 'page', 20, 170, 140, 100, { border: border(8, '#800080', 'inside') }),
  box('f2k', 'rect', 'f2', 0, 0, 140, 100, { fill: '#ffff00' }),
  box('r2', 'rect', 'page', 190, 170, 140, 100, { radius: 24, border: border(10, '#0000ff80', 'center') }),
  box('r3', 'rect', 'page', 360, 170, 140, 100, { radius: 20, border: border(10, '#000000', 'inside') }),
  box('r4', 'rect', 'page', 520, 170, 140, 100, { opacity: 0.5, border: border(6, '#ff0000', 'outside') }),
  box('r5', 'rect', 'page', 680, 170, 100, 100, { opacity: 0.5, border: border(6, '#ff0000', 'inside') }),
];
// [what, world x, world y, expected rgb]
const RED = [255, 0, 0], BLUE = [0, 0, 255], GREEN = [0, 170, 0], GREY = [204, 204, 204], WHITE = [255, 255, 255], YELLOW = [255, 255, 0];
const probes = [
  ['inside: left band', 23, 70, RED], ['inside: right band', 157, 70, RED], ['inside: top band', 90, 23, RED], ['inside: the fill', 30, 70, GREY],
  ['inside: nothing outside the box', 17, 70, WHITE], ['inside: a square corner stays square', 21.5, 21.5, RED],
  ['outside: left of the box', 187, 70, BLUE], ['outside: right of the box', 333, 70, BLUE], ['outside: above the box', 260, 17, BLUE],
  ['outside: the box is untouched', 193, 70, GREY], ['outside: nothing beyond the ring', 180, 70, WHITE],
  ['centre: on the edge', 360, 70, GREEN], ['centre: inside the ring', 368, 70, GREY], ['centre: outside the ring', 354, 70, WHITE],
  ['ellipse: the ring at 45 degrees', 375, 35, GREEN], ['ellipse: the corner of the box is empty', 362, 23, WHITE],
  ['clip: an outside border is not cut (left)', 517, 70, RED], ['clip: an outside border is not cut (right)', 663, 70, RED],
  ['clip: the child inside', 530, 70, YELLOW], ['clip: the child is cut at the edge', 510, 70, WHITE], ['clip: the child is cut at the edge (above)', 560, 10, WHITE],
  ['over the child', 24, 220, [128, 0, 128]], ['the child inside the border', 35, 220, YELLOW],
  ['alpha: over the page', 187.5, 220, [127, 127, 255]], ['alpha: over the fill', 192.5, 220, [102, 102, 230]],
  ['radius: the corner of the box is empty', 191, 171, WHITE],
  ['inside, radius 20: the ring', 365, 220, [0, 0, 0]], ['inside, radius 20: the fill', 375, 220, GREY], ['inside, radius 20: the outer corner is round', 361, 171, WHITE],
  ['inside, radius 20: the inner corner is round', 371.5, 181.5, [0, 0, 0]],
  ['opacity: the border', 517, 220, [255, 128, 128]], ['opacity: the fill', 530, 220, [230, 230, 230]],
  // Pixi fades each part on its own, CSS fades the node as one image: where an inside border lies over the fill, the canvas shows the border
  // at half over the fill at half over the page (242,115,115), the export the border alone at half over the page (255,128,128). The
  // difference is 13/255 here; the probe pins the canvas colour (tolerance 4) so a change of either side shows.
  ['opacity: an inside border over the fill is Pixi per part, not the isolated node', 683, 220, [242, 115, 115], 4],
];
// What a capture of a node with an outside border has to show: the node and the band around it, in its own pixels (scale 1).
// [what, x in the image, y in the image, expected rgb]
const capture = { id: 'r1', width: 152, height: 112, probes: [
  ['capture: the outside band on the left', 3, 56, BLUE], ['capture: the outside band above', 76, 3, BLUE], ['capture: the outside band below', 76, 108, BLUE],
  ['capture: the outside band on the right', 148, 56, BLUE], ['capture: the box', 76, 56, GREY], ['capture: just inside the band', 8, 56, GREY],
] };

const res = await fetch(`${server}/v1/files/${file}/tools/apply_commands`, {
  method: 'POST', headers: { 'content-type': 'application/json', authorization: `Bearer ${token}` },
  body: JSON.stringify({ ops: nodes.map((n, i) => ({ key: `border-${i}`, command: { type: 'addNode', node: n }, deps: [] })) }),
});
assert.equal(res.status, 200, `apply_commands: ${res.status} ${await res.clone().text()}`);

const browser = await chromium.launch({ args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
let failed = 0;
try {
  const page = await browser.newPage({ viewport: { width: 1440, height: 860 }, deviceScaleFactor: 1 });
  page.on('pageerror', (e) => { failed++; console.log('[pageerror]', e.message); });
  await page.goto(web);
  await page.waitForSelector('[data-testid=connection][data-state=live]', { timeout: 60_000 });
  await page.waitForTimeout(2500);
  const b64 = (await page.screenshot()).toString('base64');
  // The page frame is the one pure white area of the canvas: it gives the zoom and the origin, so the probes do not depend on the camera.
  const got = await page.evaluate(async ({ b64, probes }) => {
    const img = new Image();
    img.src = `data:image/png;base64,${b64}`;
    await img.decode();
    const c = document.createElement('canvas');
    c.width = img.width; c.height = img.height;
    const g = c.getContext('2d');
    g.drawImage(img, 0, 0);
    const d = g.getImageData(0, 0, c.width, c.height).data;
    let x0 = 1e9, y0 = 1e9, x1 = 0, y1 = 0;
    for (let y = 230; y < c.height - 100; y++) for (let x = 290; x < 1130; x++) {
      const i = (y * c.width + x) * 4;
      if (d[i] === 255 && d[i + 1] === 255 && d[i + 2] === 255) { x0 = Math.min(x0, x); y0 = Math.min(y0, y); x1 = Math.max(x1, x + 1); y1 = Math.max(y1, y + 1); }
    }
    const s = (x1 - x0) / 800;
    return { s, rows: probes.map(([, wx, wy]) => { const i = (Math.round(y0 + wy * s) * c.width + Math.round(x0 + wx * s)) * 4; return [d[i], d[i + 1], d[i + 2]]; }) };
  }, { b64, probes });
  assert.ok(got.s > 0.3 && got.s < 3, `the page frame was not found on the canvas (scale ${got.s})`);
  probes.forEach(([what, wx, wy, want, tol], i) => {
    const px = got.rows[i];
    const ok = px.every((v, k) => Math.abs(v - want[k]) <= (tol ?? 40));
    if (!ok) failed++;
    console.log(`${ok ? 'ok  ' : 'FAIL'} ${what} (${wx}, ${wy}): ${px.join(',')} want ${want.join(',')}`);
  });
  // The capture the agent gets, through the MCP endpoint, from the editor that is open on the file.
  let shot;
  for (let i = 0; i < 20 && !shot; i++) {
    const r = await fetch(`${server}/mcp`, {
      method: 'POST',
      headers: { 'content-type': 'application/json', accept: 'application/json, text/event-stream', authorization: `Bearer ${token}` },
      body: JSON.stringify({ jsonrpc: '2.0', id: i + 1, method: 'tools/call', params: { name: 'capture', arguments: { file, nodeId: capture.id, scale: 1 } } }),
    });
    const part = (await r.json()).result?.content?.[0];
    if (part?.type === 'image') shot = part.data;
    else await page.waitForTimeout(1000);
  }
  assert.ok(shot, 'the editor answered no capture');
  const cap = await page.evaluate(async ({ shot, probes }) => {
    const img = new Image();
    img.src = `data:image/png;base64,${shot}`;
    await img.decode();
    const c = document.createElement('canvas');
    c.width = img.width; c.height = img.height;
    const g = c.getContext('2d');
    g.drawImage(img, 0, 0);
    return { w: img.width, h: img.height, rows: probes.map(([, x, y]) => Array.from(g.getImageData(x, y, 1, 1).data.slice(0, 3))) };
  }, { shot, probes: capture.probes });
  const sized = cap.w === capture.width && cap.h === capture.height;
  if (!sized) failed++;
  console.log(`${sized ? 'ok  ' : 'FAIL'} capture: the image is the node and its outside band, ${cap.w} x ${cap.h} (want ${capture.width} x ${capture.height})`);
  capture.probes.forEach(([what, x, y, want], i) => {
    const px = cap.rows[i];
    const ok = px.every((v, k) => Math.abs(v - want[k]) <= 40);
    if (!ok) failed++;
    console.log(`${ok ? 'ok  ' : 'FAIL'} ${what} (${x}, ${y}): ${px.join(',')} want ${want.join(',')}`);
  });
} finally {
  await browser.close();
}
if (failed) process.exit(1);
console.log(`${probes.length + capture.probes.length + 1} checks pass`);
