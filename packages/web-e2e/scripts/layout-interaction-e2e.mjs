// Browser check of the interaction on a laid-out canvas against a real server (E3 layout slice 3b):
//   node packages/web-e2e/scripts/layout-interaction-e2e.mjs <web-url> <server-url>
// Three children in a column frame (their stored x and y are seeds that the layout ignores) and a hug frame with a text.
// With the host flag off a drag moves x and y as it always did. The script then turns the flag on from the page
// (`engine.setLayoutOnCanvas(true)`: the product has no switch yet) and checks the interaction rules in a real browser: a drag of
// the middle child above the first reorders (the insertion line shows while the pointer is down, the drawn order follows, one
// undo restores it, the stored x and y stay), a drag of a child out of the frame reparents, a handle on the hug frame makes
// that axis fixed, and the x and y fields of the Inspector are disabled for a child of the flow (and enabled again with the
// flag off). Start a server and the page with a file name that is new for every run, on free ports (the script refuses a file that
// already has content, and takes no default URL). The server and Vite are started as in the header of
// packages/web-variables/scripts/manage-e2e.mjs, with the ports 18081 (server) and 15174 (Vite); the token of the page is the
// file .dev-token at the root, and it is never written in a command that is kept. The server binary is
// `cargo build -p meldframe-server` (no DATABASE_URL: the files stay in memory). Then, from the root:
//   MELDFRAME_WEB_FILE=<file> node packages/web-e2e/scripts/layout-interaction-e2e.mjs http://localhost:15174 http://127.0.0.1:18081
// MELDFRAME_E2E_SHOTS=<dir> saves a screenshot while the pointer is down in each drag.
// The WASM bridge must be built from this revision (`just wasm-build`: the drop preview call is in it) and Playwright comes from
// the global module path. The web and server URLs must be loopback (the token is sent there). The engine is not on the page:
// the script serves `src/main.ts` with one added line that keeps it on `window.__mfEngine` (nothing in the repository changes).
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';

const { chromium } = createRequire(import.meta.url)('playwright');
// Both URLs are required: there is no default that could point this at the development server and its token.
const [web, server] = [process.argv[2], process.argv[3]];
if (!web || !server) throw new Error('give the web URL and the server URL (see the top of this file); there are no defaults, so the script never writes to the development server by accident');
const root = new URL('../../../', import.meta.url).pathname;
const token = readFileSync(`${root}.dev-token`, 'utf8').trim();
const file = process.env.MELDFRAME_WEB_FILE;
if (!file || file === 'file_dev') throw new Error('set MELDFRAME_WEB_FILE to a file name that is new for this run (see the top of this file)');
for (const u of [web, server]) {
  if (!['127.0.0.1', 'localhost', '[::1]'].includes(new URL(u).hostname)) throw new Error(`refusing to send the token to ${u}: the web and server URLs must be loopback (127.0.0.1, localhost or ::1)`);
}
const shots = process.env.MELDFRAME_E2E_SHOTS; // a directory: a screenshot of the pointer-down state of each drag
const auth = { authorization: `Bearer ${token}` };
const run = Math.random().toString(36).slice(2, 7);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const pageErrors = [];
let n = 0;
let browser;
let page;

async function post(command) {
  const res = await fetch(`${server}/v1/files/${file}/tools/apply_commands`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', ...auth },
    body: JSON.stringify({ ops: [{ key: `li-${run}-${++n}`, command, deps: [] }] }),
  });
  assert.equal(res.status, 200, `op refused: ${res.status} ${await res.text()}`);
}
const nodesOnServer = async () => {
  const res = await fetch(`${server}/v1/files/${file}`, { headers: auth });
  return res.status === 200 ? (await res.json()).nodes : [];
};
const nodeOf = async (id) => (await nodesOnServer()).find((x) => x.id === id);
/** The ids of the children of the frame in the order of the flow. */
const flowOf = async (parent) => (await nodesOnServer()).filter((x) => x.parentId === parent).sort((a, b) => (a.pos < b.pos ? -1 : 1)).map((x) => x.id);
async function until(what, fn, ms = 15000) {
  const t0 = Date.now();
  for (;;) {
    const v = await fn();
    if (v) return v;
    if (Date.now() - t0 > ms) throw new Error(`timed out waiting for: ${what}`);
    await sleep(150);
  }
}
const say = (text) => console.log(`       ${text}`);
const near = (a, b, tol, what) => assert.ok(Math.abs(a - b) <= tol, `${what}: ${a} is not within ${tol} of ${b}`);
const rgb = (hex) => [1, 3, 5].map((i) => Number.parseInt(hex.slice(i, i + 2), 16));
/** The box on the screen of the pixels near this colour, in the canvas or in `within` (CSS pixels: the screenshot is at scale 1), or null. */
async function bbox(hex, tol = 6, within) {
  const b64 = (await page.screenshot()).toString('base64');
  const area = within ?? (await page.locator('canvas').first().boundingBox());
  return page.evaluate(async ({ b64, want, tol, area }) => {
    const img = new Image();
    img.src = `data:image/png;base64,${b64}`;
    await img.decode();
    const c = document.createElement('canvas');
    c.width = img.width; c.height = img.height;
    const g = c.getContext('2d');
    g.drawImage(img, 0, 0);
    const d = g.getImageData(0, 0, c.width, c.height).data;
    let x0 = 1e9, y0 = 1e9, x1 = -1, y1 = -1, count = 0;
    for (let y = Math.floor(area.y); y < area.y + area.height; y++) for (let x = Math.floor(area.x); x < area.x + area.width; x++) {
      const i = (y * c.width + x) * 4;
      if (Math.abs(d[i] - want[0]) <= tol && Math.abs(d[i + 1] - want[1]) <= tol && Math.abs(d[i + 2] - want[2]) <= tol) { count++; if (x < x0) x0 = x; if (y < y0) y0 = y; if (x > x1) x1 = x; if (y > y1) y1 = y; }
    }
    return x1 < 0 ? null : { x0, y0, x1, y1, count };
  }, { b64, want: rgb(hex), tol, area });
}
const zoomOf = async () => Number.parseInt(await page.textContent('[data-testid=zoom]'), 10) / 100;
const centre = (b) => ({ x: (b.x0 + b.x1) / 2, y: (b.y0 + b.y1) / 2 });
async function dragTo(from, to, hold) {
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move((from.x + to.x) / 2, (from.y + to.y) / 2, { steps: 5 });
  await page.mouse.move(to.x, to.y, { steps: 5 });
  await sleep(150);
  try {
    if (shots) await page.screenshot({ path: `${shots}/drag-${++n}.png` });
    if (hold) await hold();
  } finally {
    await page.mouse.up(); // the gesture ends even when a check of the pointer-down state fails
  }
}
const select = (id) => page.click(`[data-testid=layer][data-id="${id}"] button >> nth=-1`);
const setLayout = (on) => page.evaluate((v) => window.__mfEngine.setLayoutOnCanvas(v), on);
/** Whether an input of the Inspector is disabled. */
const disabled = (testId) => page.locator(`[data-testid=${testId}]`).isDisabled();

const results = [];
async function check(name, fn) {
  try { await fn(); results.push(true); console.log(`ok   ${name}`); } catch (e) {
    results.push(false);
    console.log(`FAIL ${name}: ${String(e.message ?? e).split('\n')[0]}${shots ? `\n     ${String(e.stack).split('\n')[1]?.trim()}` : ''}`);
    if (shots) await page.screenshot({ path: `${shots}/failed-${results.length}.png` }).catch(() => {});
    if (shots) console.log('     events:', JSON.stringify(await page.evaluate(() => window.__events).catch(() => null)));
  }
}

// Colours that are neither the canvas nor the blue of the overlay.
const [FRAME, RED, GREEN, AMBER, PURPLE] = ['#5b4a2e', '#e5484d', '#30a46c', '#f5a623', '#8e4ec6'];
const ACCENT = '#3b82f6'; // the dark theme: the insertion line, apart from the selection outline (#60a5fa)
const F = `col_${run}`, A = `a_${run}`, B = `b_${run}`, C = `c_${run}`, H = `hug_${run}`, T = `txt_${run}`;

(async () => {
  // It writes to the file it is given: refuse one that already has content (a development file, or an earlier run).
  const present = await nodesOnServer();
  if (present.length) throw new Error(`the file ${file} already has ${present.length} nodes: use a file name that is new for this run`);
  // A column at (100, 100) with padding 20 and gap 10, three 200 x 60 children and, to its right, a hug frame with a text.
  // The stored x and y of the children are seeds: with the layout off they are where the children are drawn.
  const kid = (id, fill, y) => ({ id, name: id, type: 'rect', parentId: F, x: 20, y, w: 200, h: 60, fill });
  await post({ type: 'addNode', node: { id: F, name: `Column ${run}`, type: 'frame', parentId: null, pos: 'a0', x: 100, y: 100, w: 300, h: 400, fill: FRAME, layout: 'column', padding: 20, gap: 10 } });
  await post({ type: 'addNode', node: { ...kid(A, RED, 20), pos: 'a0' } });
  await post({ type: 'addNode', node: { ...kid(B, GREEN, 120), pos: 'a1' } });
  await post({ type: 'addNode', node: { ...kid(C, AMBER, 240), pos: 'a2' } });
  await post({ type: 'addNode', node: { id: H, name: `Hug ${run}`, type: 'frame', parentId: null, pos: 'a1', x: 700, y: 100, w: 100, h: 50, fill: PURPLE, layout: 'row', padding: 10, sw: 'hug', sh: 'hug' } });
  await post({ type: 'addNode', node: { id: T, name: `Text ${run}`, type: 'text', parentId: H, pos: 'a0', x: 0, y: 0, w: 60, h: 20, text: 'Hello flow', fontSize: 16, fill: '#ffffff', sw: 'hug', sh: 'hug' } });

  browser = await chromium.launch({ args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
  page = await browser.newPage({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1, colorScheme: 'dark' });
  if (shots) page.on('console', (m) => { if (m.text().startsWith('mf-event')) console.log('    ', m.text()); });
  page.on('pageerror', (e) => { pageErrors.push(e.message); console.log('[pageerror]', e.message); });
  await page.route('**/src/main.ts*', async (route) => {
    const res = await route.fetch();
    const body = (await res.text()).replace(/(const engine = )(Engine\.create\([^;]*\));/, '$1window.__mfEngine = $2;');
    if (!body.includes('window.__mfEngine')) throw new Error('src/main.ts no longer has `const engine = Engine.create(...)`: update the patch of this script');
    await route.fulfill({ response: res, body });
  });
  await page.goto(web);
  await until('the editor is live', async () => (await page.getAttribute('[data-testid=connection]', 'data-state')) === 'live');
  // What the app said (refusals, divergence), for the failure report.
  await page.evaluate(() => { window.__events = []; window.__mfEngine.subscribe((s) => { const names = s.events.map((e) => (e.code ? `${e.type}:${e.code}` : e.type)); window.__events.push(...names); if (names.some((x) => /diverged|refused/.test(x))) console.log('mf-event', JSON.stringify(s.events)); }); });
  await until('the nodes are drawn', async () => (await bbox(PURPLE)) && (await bbox(AMBER)));
  await sleep(400);

  await check('with the layout off a drag of a child moves its x and y by the drag, as before; one undo takes it back', async () => {
    const z = await zoomOf();
    const start = await nodeOf(B);
    const b = await bbox(GREEN);
    // Stored y 120 plus the frame at 100 plus the padding is not used: the child is drawn where its stored position says.
    near((b.y0 - (await bbox(FRAME)).y0) / z, 120, 3, 'drawn at the stored y');
    const c = centre(b);
    await dragTo(c, { x: c.x + 30, y: c.y + 20 });
    const moved = await until('the move on the server', async () => { const r = await nodeOf(B); return r.x !== start.x ? r : null; });
    near(moved.x - start.x, 30 / z, 2, 'moved x'); near(moved.y - start.y, 20 / z, 2, 'moved y');
    say(`dragged 30 x 20 px at zoom ${z}: x ${start.x} -> ${moved.x.toFixed(2)}, y ${start.y} -> ${moved.y.toFixed(2)}`);
    assert.equal(moved.parentId, F, 'still in the frame');
    assert.deepEqual(await flowOf(F), [A, B, C], 'the order did not change');
    await page.keyboard.press('Control+z');
    await until('the move undone', async () => (await nodeOf(B)).x === start.x && (await nodeOf(B)).y === start.y);
  });

  await check('the layout on: the children are drawn in the flow, the x and y fields of a child are disabled and say why', async () => {
    await setLayout(true);
    const z = await zoomOf();
    await until('the flow on the canvas', async () => { const a = await bbox(RED); const b = await bbox(GREEN); return a && b && Math.abs((b.y0 - a.y0) / z - 70) < 3; });
    await select(A);
    await until('the Inspector of A', async () => (await page.inputValue('[data-testid=prop-name]')) === A);
    assert.equal(await disabled('prop-x'), true, 'x is disabled');
    assert.equal(await disabled('prop-y'), true, 'y is disabled');
    assert.equal(await disabled('prop-w'), false, 'the width is not');
    const note = page.locator('#prop-position-note');
    assert.match(await note.textContent(), /auto layout/);
    assert.equal(await page.getAttribute('[data-testid=prop-x]', 'aria-describedby'), 'prop-position-note');
    // The fields show where the child is drawn: the frame at 100 plus the padding 20.
    near(Number(await page.inputValue('[data-testid=prop-y]')), 20, 0.5, 'drawn y in the frame');
    await select(F);
    await until('the Inspector of the frame', async () => (await page.inputValue('[data-testid=prop-name]')).startsWith('Column'));
    assert.equal(await disabled('prop-x'), false, 'a frame on the page keeps its fields');
    assert.equal(await page.locator('#prop-position-note').count(), 0);
  });

  await check('dragging the middle child above the first reorders: the line shows, the drawn order follows, the stored x and y stay, one undo restores it', async () => {
    const z = await zoomOf();
    const before = await nodeOf(B);
    const a = await bbox(RED);
    const c = centre(await bbox(GREEN));
    const target = { x: c.x, y: a.y0 + 8 }; // inside the frame, above the middle of the first child
    await dragTo(c, target, async () => {
      // The line is at the top edge of the first child (the gap before it is the padding, 20 px): look in a band around it.
      const f = await bbox(FRAME);
      const line = await bbox(ACCENT, 18, { x: f.x0, y: a.y0 - 14, width: f.x1 - f.x0 + 1, height: 20 });
      assert.ok(line, 'the insertion line is drawn');
      assert.ok(line.count > 100, `the line is long enough: ${line.count} px`);
      assert.ok(line.y1 - line.y0 <= 6, `and thin: ${line.y1 - line.y0} px`);
      near((line.x1 - line.x0 + 1) / z, 200, 4, 'and as wide as the neighbour');
      say(`insertion line while the pointer is down: ${line.x1 - line.x0 + 1} x ${line.y1 - line.y0 + 1} px, ${line.count} px of accent, ${(a.y0 - line.y0).toFixed(1)} px above the first child`);
    });
    say('stored x, y of the moved child before the drop: ' + JSON.stringify([before.x, before.y]));
    assert.deepEqual(await until('the new order', async () => { const o = await flowOf(F); return o[0] === B ? o : null; }), [B, A, C]);
    const now = await nodeOf(B);
    assert.deepEqual([now.x, now.y], [before.x, before.y], 'the stored x and y of the child are left alone');
    await until('the drawn order follows', async () => { const r = await bbox(RED); const g = await bbox(GREEN); const m = await bbox(AMBER); return g.y0 < r.y0 && r.y0 < m.y0; });
    const f = await bbox(FRAME);
    const left = await bbox(ACCENT, 18, { x: f.x0, y: a.y0 - 14, width: f.x1 - f.x0 + 1, height: 20 });
    assert.ok(!left || left.count < 20, `the line is gone after the drop: ${left?.count} px`);
    await page.keyboard.press('Control+z');
    assert.deepEqual(await until('one undo', async () => { const o = await flowOf(F); return o[0] === A ? o : null; }), [A, B, C]);
    await until('the drawn order is back', async () => { const r = await bbox(RED); const g = await bbox(GREEN); return r.y0 < g.y0; });
  });

  await check('dragging a child out of the frame onto the page reparents it where it was let go; one undo puts it back in its place', async () => {
    const z = await zoomOf();
    const f = await bbox(FRAME);
    const h = await bbox(PURPLE);
    const c = centre(await bbox(AMBER));
    const target = { x: (f.x1 + h.x0) / 2, y: c.y - 40 };
    const drawn = await bbox(AMBER);
    await dragTo(c, target);
    const out = await until('the reparent on the server', async () => { const r = await nodeOf(C); return r.parentId === null ? r : null; });
    // The node keeps the place it was drawn at plus the drag, now in the space of the page (world = screen over the zoom).
    say(`reparented to the page at x ${out.x.toFixed(1)}, y ${out.y.toFixed(1)} (drawn at 120, 260, dragged ${((target.x - c.x) / z).toFixed(1)} x ${((target.y - c.y) / z).toFixed(1)} world units)`);
    near(out.x - 120, (target.x - c.x) / z, 3, 'x: where it was drawn, plus the drag');
    near(out.y - (100 + 20 + 70 + 70), (target.y - c.y) / z, 3, 'y: where it was drawn, plus the drag');
    assert.deepEqual(await flowOf(F), [A, B]);
    await until('drawn where it was dropped', async () => { const m = await bbox(AMBER); return m && Math.abs(m.x0 - drawn.x0 - (target.x - c.x)) <= 3; });
    await page.keyboard.press('Control+z');
    const back = await until('one undo', async () => { const r = await nodeOf(C); return r.parentId === F ? r : null; });
    assert.deepEqual([back.x, back.y], [20, 240], 'its stored position is as it was');
    assert.deepEqual(await flowOf(F), [A, B, C]);
  });

  await check('a handle on a hug frame makes that axis fixed and leaves the other one hug; one undo restores both', async () => {
    const z = await zoomOf();
    await select(H);
    await until('the Inspector of the hug frame', async () => (await page.inputValue('[data-testid=prop-name]')).startsWith('Hug'));
    const sizing = (axis) => page.locator(`[data-testid=prop-${axis}] [role=radio][aria-checked=true]`).textContent();
    assert.deepEqual([(await sizing('sw')).trim(), (await sizing('sh')).trim()], ['Hug', 'Hug']);
    const box = await bbox(PURPLE);
    // The drawn width, from the scene the canvas draws (the pixels of the frame are covered by its outline at the edges).
    const width = await page.evaluate((id) => window.__mfEngine.snapshot().scene.nodes.find((x) => x.id === id).w, H);
    await dragTo({ x: box.x1 + 0.5, y: (box.y0 + box.y1) / 2 }, { x: box.x1 + 40.5, y: (box.y0 + box.y1) / 2 });
    const hug = await until('the resize on the server', async () => { const r = await nodeOf(H); return r.sw === 'fixed' ? r : null; });
    assert.equal(hug.sh, 'hug', 'the height was not touched');
    say(`hug frame: drawn width ${width.toFixed(2)}, dragged 40 px (${(40 / z).toFixed(2)} world units): width ${hug.w.toFixed(2)}, sw ${hug.sw}, sh ${hug.sh}`);
    near(hug.w, width + 40 / z, 2, 'the width is the drawn width plus the drag');
    await until('the Inspector says fixed width and hug height', async () => (await sizing('sw')).trim() === 'Fixed' && (await sizing('sh')).trim() === 'Hug');
    await page.keyboard.press('Control+z');
    await until('one undo', async () => { const r = await nodeOf(H); return r.sw === 'hug' && r.sh === 'hug'; });
  });

  await check('the layout off again: the children are drawn where they are stored and the fields are enabled', async () => {
    await setLayout(false);
    const z = await zoomOf();
    await until('the stored boxes on the canvas', async () => { const f = await bbox(FRAME); const b = await bbox(GREEN); return f && b && Math.abs((b.y0 - f.y0) / z - 120) < 3; });
    await select(A);
    await until('the Inspector of A', async () => (await page.inputValue('[data-testid=prop-name]')) === A);
    assert.equal(await disabled('prop-x'), false);
    assert.equal(await page.locator('#prop-position-note').count(), 0);
  });

  assert.deepEqual(pageErrors, [], 'no page errors');
})()
  .catch((e) => { results.push(false); console.log('FAIL (setup)', e.stack ?? e); })
  .finally(async () => {
    await browser?.close();
    const failed = results.filter((r) => !r).length;
    console.log(`${results.length - failed} of ${results.length} passed${pageErrors.length ? `, ${pageErrors.length} page error(s)` : ''}`);
    process.exit(failed || pageErrors.length ? 1 : 0);
  });
