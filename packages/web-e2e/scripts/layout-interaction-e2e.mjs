// Browser check of the interaction on a laid-out canvas against a real server (E3 layout, the whole slice):
//   node packages/web-e2e/scripts/layout-interaction-e2e.mjs <web-url> <server-url>
// Three children in a column frame (their stored x and y are seeds that the layout ignores) and a hug frame with a text.
// The page is opened with /?canvaslayout=0, so the layout is off: a drag moves x and y as it always did. The script then turns
// "Auto layout on canvas" on the way a person does (the command palette, Ctrl+K) and checks the interaction rules in a real
// browser: a drag of the middle child above the first reorders (the insertion line shows while the pointer is down, the drawn
// order follows, one undo restores it, the stored x and y stay), a drag of a child out of the frame reparents, a handle on the
// hug frame makes that axis fixed, a north-west handle on a child of the flow keeps the drawn origin while the pointer is down
// and after the release, an arrow key along the flow moves the selected child one place (the key across it does nothing), a
// drag of two selected children moves them as one block in order, a drag of a child with a node that is not in a flow moves
// the one by the flow and the other by the drag, and the x and y fields of the Inspector are disabled for a child of the flow
// (and enabled again with the layout off). The choice is remembered in the browser (a reload keeps it), /?canvaslayout=0 forces
// it off and a browser with no choice starts with it on. The page needs no patch: nothing in the repository changes.
// Start a server and the page with a file name that is new for every run, on free ports (the script refuses a file that
// already has content, and takes no default URL). The server and Vite are started as in the header of
// packages/web-variables/scripts/manage-e2e.mjs, with the ports 18081 (server) and 15174 (Vite); the page's token is read from
// the file .dev-token at the root, and it is never written in a command that is kept. The server binary is built with
// `cargo build -p meldframe-server` (no DATABASE_URL: the files stay in memory). Then, from the root, with MELDFRAME_WEB_FILE set
// to the new file name:
//   node packages/web-e2e/scripts/layout-interaction-e2e.mjs http://localhost:15174 http://127.0.0.1:18081
// MELDFRAME_E2E_SHOTS=<dir> saves a screenshot while the pointer is down in each drag.
// MELDFRAME_UI=react opens every page with `react=canvas` (the React copy of the canvas, E7.3 slice S11; MELDFRAME_REACT_PANELS adds switch words, such
// as `strict` for React's strict mode in development): a first check asks that React drew the canvas (a fibre on its host, so a silent fallback to
// Svelte fails), every other check is the same on both, and a last check blocks the module of the React canvas and asks that the Svelte canvas is
// drawn and the layout still shows (the match is on the paths of the Vite dev server's modules: dev server only).
// The WASM bridge must be built from this revision (`just wasm-build`) and Playwright comes from the global module path. The web
// and server URLs must be loopback (the token is sent there).
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { withinReadout } from './zoom.mjs';

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
const react = process.env.MELDFRAME_UI === 'react';
/** A page of the editor, with the React copy of the canvas when MELDFRAME_UI=react. */
const urlOf = (query = '') => {
  const q = [query, react ? `react=${['canvas', ...(process.env.MELDFRAME_REACT_PANELS ?? '').split(',').filter(Boolean)].join(',')}` : ''].filter(Boolean).join('&');
  return q ? `${web}/?${q}` : web;
};
const fibre = (p) => p.evaluate(() => Object.keys(document.querySelector('[role=application]') ?? {}).some((k) => k.startsWith('__reactFiber$')));
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
// A distance on the canvas and the same distance on the screen: the zoom readout is a whole percent, so the real zoom is only
// known to within half a percent (see zoom.mjs; issue 226). `world` is a distance on the canvas, `pixels` one on the screen.
const nearZoom = (world, pixels, readout, tol, what) =>
  assert.ok(withinReadout(world, pixels, readout, tol), `${what}: ${world} is not within ${tol} of ${pixels} px at a zoom of ${readout} (readout, +-0.005)`);
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
/** Turns "Auto layout on canvas" on or off with the command palette, and says what the palette showed for it. */
async function toggleLayout() {
  await page.keyboard.press('Control+k');
  await page.fill('[data-testid=palette-input]', 'auto layout');
  const row = page.locator('[data-testid=palette-option]').first();
  await row.waitFor();
  const text = await row.textContent();
  await page.keyboard.press('Enter');
  await page.locator('[data-testid=palette]').waitFor({ state: 'detached' });
  // The page says what the switch is now, in a live region.
  const now = /Off/.test(text) ? 'on' : 'off';
  await page.locator('[data-testid=toast-region] [data-testid=notice]', { hasText: `Auto layout on canvas: ${now}` }).waitFor();
  return text;
}
/** The click that selects a node on the canvas: the focus stays on the canvas, so the keys reach the editor. */
const clickNode = async (hex, mods = []) => {
  const c = centre(await bbox(hex));
  for (const m of mods) await page.keyboard.down(m);
  await page.mouse.click(c.x, c.y);
  for (const m of mods) await page.keyboard.up(m);
};
const stored = async (id) => { const r = await nodeOf(id); return [r.x, r.y]; };
/** Whether an input of the Inspector is disabled. */
const disabled = (testId) => page.locator(`[data-testid=${testId}]`).isDisabled();

const results = [];
async function check(name, fn) {
  try { await fn(); results.push(true); console.log(`ok   ${name}`); } catch (e) {
    results.push(false);
    console.log(`FAIL ${name}: ${String(e.message ?? e).split('\n')[0]}${shots ? `\n     ${String(e.stack).split('\n')[1]?.trim()}` : ''}`);
    if (shots) await page.screenshot({ path: `${shots}/failed-${results.length}.png` }).catch(() => {});
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
  page.on('pageerror', (e) => { pageErrors.push(e.message); console.log('[pageerror]', e.message); });
  await page.goto(urlOf('canvaslayout=0'));
  await until('the editor is live', async () => (await page.getAttribute('[data-testid=connection]', 'data-state')) === 'live');
  await until('the nodes are drawn', async () => (await bbox(PURPLE)) && (await bbox(AMBER)));
  await sleep(400);

  await check(`the canvas is drawn by ${react ? 'React' : 'Svelte'}`, async () => {
    assert.equal(await fibre(page), react, react ? 'the host has no React fibre: the page fell back to Svelte' : 'the host has a React fibre on the Svelte run');
  });

  await check('with the layout off a drag of a child moves its x and y by the drag, as before; one undo takes it back', async () => {
    const z = await zoomOf();
    const start = await nodeOf(B);
    const b = await bbox(GREEN);
    // Stored y 120 plus the frame at 100 plus the padding is not used: the child is drawn where its stored position says.
    nearZoom(120, b.y0 - (await bbox(FRAME)).y0, z, 3, 'drawn at the stored y');
    const c = centre(b);
    await dragTo(c, { x: c.x + 30, y: c.y + 20 });
    const moved = await until('the move on the server', async () => { const r = await nodeOf(B); return r.x !== start.x ? r : null; });
    nearZoom(moved.x - start.x, 30, z, 2, 'moved x'); nearZoom(moved.y - start.y, 20, z, 2, 'moved y');
    say(`dragged 30 x 20 px at zoom ${z}: x ${start.x} -> ${moved.x.toFixed(2)}, y ${start.y} -> ${moved.y.toFixed(2)}`);
    assert.equal(moved.parentId, F, 'still in the frame');
    assert.deepEqual(await flowOf(F), [A, B, C], 'the order did not change');
    await page.keyboard.press('Control+z');
    await until('the move undone', async () => (await nodeOf(B)).x === start.x && (await nodeOf(B)).y === start.y);
  });

  await check('the layout on: the children are drawn in the flow, the x and y fields of a child are disabled and say why', async () => {
    assert.match(await toggleLayout(), /Off/, 'the palette says it is off, as the page was opened with ?canvaslayout=0');
    assert.equal(await page.evaluate(() => localStorage.getItem('meldframe.autoLayoutOnCanvas')), 'on', 'the choice is remembered');
    const z = await zoomOf();
    await until('the flow on the canvas', async () => { const a = await bbox(RED); const b = await bbox(GREEN); return a && b && withinReadout(70, b.y0 - a.y0, z, 3); });
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
      nearZoom(200, line.x1 - line.x0 + 1, z, 4, 'and as wide as the neighbour');
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
    // The drop is two ops (the reparent, then the place): wait for the second, or the poll can read the node between them.
    const out = await until('the reparent on the server', async () => { const r = await nodeOf(C); return r.parentId === null && (r.x !== 20 || r.y !== 240) ? r : null; });
    // The node keeps the place it was drawn at plus the drag, now in the space of the page (world = screen over the zoom).
    say(`reparented to the page at x ${out.x.toFixed(1)}, y ${out.y.toFixed(1)} (drawn at 120, 260, dragged ${((target.x - c.x) / z).toFixed(1)} x ${((target.y - c.y) / z).toFixed(1)} world units)`);
    nearZoom(out.x - 120, target.x - c.x, z, 3, 'x: where it was drawn, plus the drag');
    nearZoom(out.y - (100 + 20 + 70 + 70), target.y - c.y, z, 3, 'y: where it was drawn, plus the drag');
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
    // The drawn width, as the Inspector shows it with the layout on (the pixels of the frame are covered by its outline at the edges).
    const width = Number(await page.inputValue('[data-testid=prop-w]'));
    await dragTo({ x: box.x1 + 0.5, y: (box.y0 + box.y1) / 2 }, { x: box.x1 + 40.5, y: (box.y0 + box.y1) / 2 });
    const hug = await until('the resize on the server', async () => { const r = await nodeOf(H); return r.sw === 'fixed' ? r : null; });
    assert.equal(hug.sh, 'hug', 'the height was not touched');
    say(`hug frame: drawn width ${width.toFixed(2)}, dragged 40 px (${(40 / z).toFixed(2)} world units): width ${hug.w.toFixed(2)}, sw ${hug.sw}, sh ${hug.sh}`);
    nearZoom(hug.w - width, 40, z, 2, 'the width is the drawn width plus the drag');
    await until('the Inspector says fixed width and hug height', async () => (await sizing('sw')).trim() === 'Fixed' && (await sizing('sh')).trim() === 'Hug');
    await page.keyboard.press('Control+z');
    await until('one undo', async () => { const r = await nodeOf(H); return r.sw === 'hug' && r.sh === 'hug'; });
  });

  await check('a north-west handle on a child of the flow keeps the drawn origin while the pointer is down and after the release', async () => {
    const z = await zoomOf();
    const before = await bbox(GREEN);
    const start = await nodeOf(B);
    await clickNode(GREEN);
    await until('B is selected', async () => (await page.inputValue('[data-testid=prop-name]')) === B);
    // The handle sits on the north-west corner of the box; the pointer goes up and left, which would move the origin if the
    // preview followed it. The app layer says the position is the layout's, so the preview only grows the box to the right and down.
    await dragTo({ x: before.x0, y: before.y0 }, { x: before.x0 - 24, y: before.y0 - 12 }, async () => {
      const held = await bbox(GREEN);
      near(held.x0, before.x0, 2, 'the left edge while the pointer is down');
      near(held.y0, before.y0, 2, 'the top edge while the pointer is down');
      near((held.x1 - held.x0) - (before.x1 - before.x0), 24, 3, 'the box is wider by the drag');
      // The box below it (the next child, drawn over it) hides the last pixels of the growth, so only a range is checked.
      const taller = (held.y1 - held.y0) - (before.y1 - before.y0);
      assert.ok(taller >= 5 && taller <= 14, `and taller by the drag: ${taller}`);
      say(`while the pointer is down: left edge ${held.x0} (was ${before.x0}), top ${held.y0} (was ${before.y0}), ${held.x1 - held.x0 + 1} x ${held.y1 - held.y0 + 1} px (was ${before.x1 - before.x0 + 1} x ${before.y1 - before.y0 + 1})`);
    });
    const done = await until('the resize on the server', async () => { const r = await nodeOf(B); return r.w !== start.w ? r : null; });
    nearZoom(done.w - start.w, 24, z, 2, 'the width grew by the drag');
    nearZoom(done.h - start.h, 12, z, 2, 'the height grew by the drag');
    assert.deepEqual([done.x, done.y], [start.x, start.y], 'the stored x and y are left alone');
    await until('the box is where the flow puts it', async () => { const after = await bbox(GREEN); return Math.abs(after.x0 - before.x0) <= 2 && Math.abs(after.y0 - before.y0) <= 2; });
    await page.keyboard.press('Control+z');
    await until('one undo', async () => (await nodeOf(B)).w === start.w);
    await page.mouse.click(5, 5); // clear the selection
  });

  await check('an arrow key along the flow moves the selected child one place; the key across it does nothing; one undo each', async () => {
    const start = await stored(B);
    await clickNode(GREEN);
    await until('B is selected', async () => (await page.inputValue('[data-testid=prop-name]')) === B);
    await page.keyboard.press('ArrowLeft'); // across the column
    await page.keyboard.press('ArrowRight');
    await sleep(300);
    assert.deepEqual(await flowOf(F), [A, B, C], 'the keys across the column do nothing');
    await page.keyboard.press('ArrowUp');
    assert.deepEqual(await until('B first', async () => { const o = await flowOf(F); return o[0] === B ? o : null; }), [B, A, C]);
    await until('drawn first', async () => { const g = await bbox(GREEN); const r = await bbox(RED); return g && r && g.y0 < r.y0; });
    await page.keyboard.press('ArrowUp'); // already first
    await page.keyboard.press('ArrowDown');
    await page.keyboard.press('ArrowDown');
    assert.deepEqual(await until('B last', async () => { const o = await flowOf(F); return o[2] === B ? o : null; }), [A, C, B]);
    assert.deepEqual(await stored(B), start, 'the stored x and y are left alone');
    await page.keyboard.press('Control+z');
    assert.deepEqual(await until('one undo is one key', async () => { const o = await flowOf(F); return o[1] === B ? o : null; }), [A, B, C]);
    await page.keyboard.press('Control+z');
    await page.keyboard.press('Control+z');
    await until('back as it was', async () => (await flowOf(F)).join() === [A, B, C].join());
    await page.mouse.click(5, 5);
  });

  await check('two selected children dragged below the third move as one block in order, with no stored position written; one undo', async () => {
    const before = [await stored(A), await stored(B)];
    await clickNode(RED);
    await clickNode(GREEN, ['Shift']);
    const c = centre(await bbox(RED));
    const last = await bbox(AMBER);
    const target = { x: c.x, y: last.y1 - 4 }; // inside the frame, past the middle of the third child
    await dragTo(c, target, async () => {
      const f = await bbox(FRAME);
      const line = await bbox(ACCENT, 18, { x: f.x0, y: last.y1 - 14, width: f.x1 - f.x0 + 1, height: 40 });
      assert.ok(line && line.count > 100, 'the insertion line shows below the third child while the pointer is down');
    });
    assert.deepEqual(await until('the block moved', async () => { const o = await flowOf(F); return o[0] === C ? o : null; }), [C, A, B]);
    assert.deepEqual([await stored(A), await stored(B)], before, 'no stored position of a flow child is written');
    await until('drawn in that order', async () => { const [a, b, m] = [await bbox(RED), await bbox(GREEN), await bbox(AMBER)]; return m.y0 < a.y0 && a.y0 < b.y0; });
    await page.keyboard.press('Control+z');
    assert.deepEqual(await until('one undo', async () => { const o = await flowOf(F); return o[0] === A ? o : null; }), [A, B, C]);
    await page.mouse.click(5, 5);
  });

  await check('a child dragged with a node that is not in a flow: the child goes to the slot, the other moves by the drag', async () => {
    const z = await zoomOf();
    const start = await nodeOf(H);
    const was = await stored(A);
    // The frame is picked in the Layers panel (a click in the middle of it would pick its text), with Shift to add it.
    await select(A);
    await page.click(`[data-testid=layer][data-id="${H}"] button >> nth=-1`, { modifiers: ['Shift'] });
    await until('both are selected', async () => (await page.locator('[data-testid=layer][data-selected=true]').count()) === 2);
    const c = centre(await bbox(RED));
    const last = await bbox(AMBER);
    const target = { x: c.x, y: last.y1 - 4 };
    await dragTo(c, target);
    assert.deepEqual(await until('the child moved in the flow', async () => { const o = await flowOf(F); return o[2] === A ? o : null; }), [B, C, A]);
    const moved = await until('the frame moved by the drag', async () => { const r = await nodeOf(H); return r.x !== start.x || r.y !== start.y ? r : null; });
    nearZoom(moved.x - start.x, target.x - c.x, z, 2, 'x moved by the drag');
    nearZoom(moved.y - start.y, target.y - c.y, z, 2, 'y moved by the drag');
    assert.deepEqual(await stored(A), was, 'the flow child keeps its stored x and y');
    await page.keyboard.press('Control+z');
    await until('one undo for both', async () => { const r = await nodeOf(H); return r.x === start.x && r.y === start.y && (await flowOf(F)).join() === [A, B, C].join(); });
    await page.mouse.click(5, 5);
  });

  await check('the layout off again: the children are drawn where they are stored and the fields are enabled', async () => {
    assert.match(await toggleLayout(), /On/, 'the palette says it is on');
    assert.equal(await page.evaluate(() => localStorage.getItem('meldframe.autoLayoutOnCanvas')), 'off', 'the choice is remembered');
    const z = await zoomOf();
    await until('the stored boxes on the canvas', async () => { const f = await bbox(FRAME); const b = await bbox(GREEN); return f && b && withinReadout(120, b.y0 - f.y0, z, 3); });
    await select(A);
    await until('the Inspector of A', async () => (await page.inputValue('[data-testid=prop-name]')) === A);
    assert.equal(await disabled('prop-x'), false);
    assert.equal(await page.locator('#prop-position-note').count(), 0);
  });

  await check('the choice is remembered: a reload starts laid out after it was turned on, ?canvaslayout=0 forces it off and ?canvaslayout=1 on', async () => {
    const z = async () => zoomOf();
    const laidOut = async () => { const a = await bbox(RED); const b = await bbox(GREEN); return a && b && withinReadout(70, b.y0 - a.y0, await z(), 3); };
    assert.match(await toggleLayout(), /Off/); // on again, remembered
    await page.goto(urlOf());
    await until('the editor is live', async () => (await page.getAttribute('[data-testid=connection]', 'data-state')) === 'live');
    await until('laid out after a reload with no parameter', laidOut);
    await page.goto(urlOf('canvaslayout=0'));
    await until('the editor is live again', async () => (await page.getAttribute('[data-testid=connection]', 'data-state')) === 'live');
    await until('the stored boxes with ?canvaslayout=0', async () => { const f = await bbox(FRAME); const b = await bbox(GREEN); return f && b && withinReadout(120, b.y0 - f.y0, await z(), 3); });
    assert.equal(await page.evaluate(() => localStorage.getItem('meldframe.autoLayoutOnCanvas')), 'on', 'a forced page does not change the choice');
    await page.evaluate(() => localStorage.setItem('meldframe.autoLayoutOnCanvas', 'off'));
    await page.goto(urlOf('canvaslayout=1'));
    await until('laid out with ?canvaslayout=1 over a choice of off', async () => { await sleep(150); return laidOut(); });
    await page.evaluate(() => localStorage.removeItem('meldframe.autoLayoutOnCanvas'));
    await page.goto(urlOf());
    await until('laid out with no choice at all: the default is on', async () => { await sleep(150); return laidOut(); });
  });

  if (react) {
    await check('the chunk of the React canvas does not load: the Svelte canvas is drawn instead, laid out, and a drag still reorders', async () => {
      const blocked = await browser.newPage({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1, colorScheme: 'dark' });
      blocked.on('pageerror', (e) => { pageErrors.push(e.message); console.log('[pageerror]', e.message); });
      await blocked.route('**/canvas/react/Canvas.tsx*', (r) => r.abort());
      const kept = page;
      page = blocked; // the helpers read the page of the script
      try {
        await page.goto(urlOf());
        await until('the editor is live', async () => (await page.getAttribute('[data-testid=connection]', 'data-state')) === 'live');
        await until('the canvas of the fallback is drawn', async () => (await bbox(RED)) && (await bbox(GREEN)));
        assert.equal(await fibre(page), false, 'the host has a React fibre although its module was blocked');
        const a = await bbox(RED);
        const b = await bbox(GREEN);
        assert.ok(withinReadout(70, b.y0 - a.y0, await zoomOf(), 3), 'the children are drawn in the flow');
        const before = await flowOf(F);
        assert.deepEqual(before, [A, B, C], 'the checks before this one left the order as it was made');
        const mid = await bbox(GREEN);
        const first = await bbox(RED);
        await page.mouse.move((mid.x0 + mid.x1) / 2, (mid.y0 + mid.y1) / 2);
        await page.mouse.down();
        await page.mouse.move((first.x0 + first.x1) / 2, first.y0 + 5, { steps: 8 });
        await page.mouse.up();
        await until('the order on the server', async () => (await flowOf(F)).join() !== before.join());
        assert.deepEqual(await flowOf(F), [B, A, C]);
        await page.keyboard.press('Control+z');
        await until('the order undone', async () => (await flowOf(F)).join() === before.join());
      } finally {
        page = kept;
        await blocked.close();
      }
    });
  }

  assert.deepEqual(pageErrors, [], 'no page errors');
})()
  .catch((e) => { results.push(false); console.log('FAIL (setup)', e.stack ?? e); })
  .finally(async () => {
    await browser?.close();
    const failed = results.filter((r) => !r).length;
    console.log(`${results.length - failed} of ${results.length} passed${pageErrors.length ? `, ${pageErrors.length} page error(s)` : ''}`);
    process.exit(failed || pageErrors.length ? 1 : 0);
  });
