// Browser check of the editor against a real server: node packages/web-e2e/scripts/editor-e2e.mjs [web-url] [server-url]
// Needs `just dev` (server on 8080) and `just web` (Vite on 5173) running, the WASM bridge built (`just wasm-build`),
// and Playwright from the global module path. It restarts the server (bash scripts/dev.sh) for the reconnect check, so
// run it against the development server only. Not part of `npm test`: it needs a browser, a server and Postgres.
// MELDFRAME_UI=react opens the page with `react=feedback,layers`: the notices of its checks (a refused delete, a missing image) are then
// drawn by the React copy of packages/web-feedback, and the Layers panel by the React copy of packages/web-panels (S5). The banners and the
// other states are checked by packages/web-feedback/scripts/feedback-e2e.mjs, the Layers panel by packages/web-panels/scripts/layers-e2e.mjs.
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { closeSync, openSync, readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { createHash } from 'node:crypto';
import { deflateSync } from 'node:zlib';
import { withinReadout } from './zoom.mjs';

const { chromium } = createRequire(import.meta.url)('playwright');
const web = process.argv[2] ?? 'http://localhost:5173';
const server = process.argv[3] ?? 'http://127.0.0.1:8080';
const root = new URL('../../../', import.meta.url).pathname;
const token = readFileSync(`${root}.dev-token`, 'utf8').trim();
const file = process.env.MELDFRAME_WEB_FILE ?? 'file_dev';
const react = process.env.MELDFRAME_UI === 'react';
const run = Math.random().toString(36).slice(2, 8);
const pageErrors = [];
let browser;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let n = 0;

async function post(command) {
  const res = await fetch(`${server}/v1/files/${file}/tools/apply_commands`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', authorization: `Bearer ${token}` },
    body: JSON.stringify({ ops: [{ key: `e2e-${run}-${++n}`, command, deps: [] }] }),
  });
  assert.equal(res.status, 200, `op refused: ${res.status} ${await res.text()}`);
}

const rgb = (hex) => [1, 3, 5].map((i) => Number.parseInt(hex.slice(i, i + 2), 16));
/** How many pixels of the screenshot are (nearly) this colour. */
async function count(page, hex) {
  const b64 = (await page.screenshot()).toString('base64');
  return page.evaluate(async ({ b64, want }) => {
    const img = new Image();
    img.src = `data:image/png;base64,${b64}`;
    await img.decode();
    const c = document.createElement('canvas');
    c.width = img.width; c.height = img.height;
    const g = c.getContext('2d');
    g.drawImage(img, 0, 0);
    const d = g.getImageData(0, 0, c.width, c.height).data;
    let k = 0;
    for (let i = 0; i < d.length; i += 4) if (Math.abs(d[i] - want[0]) < 6 && Math.abs(d[i + 1] - want[1]) < 6 && Math.abs(d[i + 2] - want[2]) < 6) k++;
    return k;
  }, { b64, want: rgb(hex) });
}
async function until(what, fn, ms = 20000) {
  const t0 = Date.now();
  for (;;) {
    const v = await fn();
    if (v) return v;
    if (Date.now() - t0 > ms) throw new Error(`timed out waiting for: ${what}`);
    await sleep(200);
  }
}
const state = (page) => page.getAttribute('[data-testid=connection]', 'data-state');

/** The file on the server as the nodes with their sizes. */
async function nodesOnServer() {
  const res = await fetch(`${server}/v1/files/${file}`, { headers: { authorization: `Bearer ${token}` } });
  assert.equal(res.status, 200);
  return (await res.json()).nodes;
}
const nodeOf = async (nid) => (await nodesOnServer()).find((x) => x.id === nid);
/** The box on the screen of the pixels of this colour (CSS pixels: the screenshot is at scale 1). */
async function bbox(page, hex) {
  const b64 = (await page.screenshot()).toString('base64');
  return page.evaluate(async ({ b64, want }) => {
    const img = new Image();
    img.src = `data:image/png;base64,${b64}`;
    await img.decode();
    const c = document.createElement('canvas');
    c.width = img.width; c.height = img.height;
    const g = c.getContext('2d');
    g.drawImage(img, 0, 0);
    const d = g.getImageData(0, 0, c.width, c.height).data;
    let x0 = 1e9, y0 = 1e9, x1 = -1, y1 = -1;
    for (let y = 0; y < c.height; y++) for (let x = 0; x < c.width; x++) {
      const i = (y * c.width + x) * 4;
      if (Math.abs(d[i] - want[0]) < 6 && Math.abs(d[i + 1] - want[1]) < 6 && Math.abs(d[i + 2] - want[2]) < 6) { if (x < x0) x0 = x; if (y < y0) y0 = y; if (x > x1) x1 = x; if (y > y1) y1 = y; }
    }
    return x1 < 0 ? null : { x0, y0, x1, y1 };
  }, { b64, want: rgb(hex) });
}
const zoomOf = async (page) => Number.parseInt(await page.textContent('[data-testid=zoom]'), 10) / 100;

/** A PNG of `w` x `h` pixels from `rowColour(y)` (a row is one colour): enough to tell where each fit puts an image. */
function pngOf(w, h, rowColour) {
  const crcTable = Array.from({ length: 256 }, (_, n) => { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; return c >>> 0; });
  const crc = (buf) => { let c = 0xffffffff; for (const b of buf) c = crcTable[(c ^ b) & 255] ^ (c >>> 8); return (c ^ 0xffffffff) >>> 0; };
  const chunk = (type, data) => {
    const len = Buffer.alloc(4); len.writeUInt32BE(data.length);
    const body = Buffer.concat([Buffer.from(type), data]);
    const sum = Buffer.alloc(4); sum.writeUInt32BE(crc(body));
    return Buffer.concat([len, body, sum]);
  };
  const head = Buffer.alloc(13); head.writeUInt32BE(w, 0); head.writeUInt32BE(h, 4); head[8] = 8; head[9] = 2;
  const raw = Buffer.alloc((w * 3 + 1) * h);
  for (let y = 0; y < h; y++) { const o = y * (w * 3 + 1); raw[o] = 0; const [r, g, b] = rgb(rowColour(y)); for (let x = 0; x < w; x++) { raw[o + 1 + x * 3] = r; raw[o + 2 + x * 3] = g; raw[o + 3 + x * 3] = b; } }
  return Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk('IHDR', head), chunk('IDAT', deflateSync(raw)), chunk('IEND', Buffer.alloc(0))]);
}
/** Uploads an image to the workspace of the token and returns its id. */
async function upload(bytes) {
  const res = await fetch(`${server}/v1/assets`, { method: 'POST', headers: { authorization: `Bearer ${token}` }, body: bytes });
  const body = await res.json();
  assert.equal(res.status, 200, `upload refused: ${res.status} ${JSON.stringify(body)}`);
  assert.equal(body.id, createHash('sha256').update(bytes).digest('hex'));
  return body.id;
}
async function drag(page, from, to) {
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move((from.x + to.x) / 2, (from.y + to.y) / 2, { steps: 4 });
  await page.mouse.move(to.x, to.y, { steps: 4 });
  await page.mouse.up();
}
const near = (a, b, tol, what) => assert.ok(Math.abs(a - b) <= tol, `${what}: ${a} is not within ${tol} of ${b}`);
// A distance on the canvas from a distance on the screen: the zoom readout is a whole percent, so the real zoom is only known to
// within half a percent and the expected distance to the range that gives (issue 226).
const nearZoom = (actual, pixels, readout, tol, what) =>
  assert.ok(withinReadout(actual, pixels, readout, tol), `${what}: ${actual} is not within ${tol} of ${pixels} px at a zoom of ${readout} (readout, +-0.005)`);

const results = [];
async function check(name, fn) {
  try { await fn(); results.push(true); console.log('ok  ', name); }
  catch (e) { results.push(false); console.log('FAIL', name, '\n    ', String(e.message ?? e).split('\n')[0]); }
}

const A = '#12ab34', B = '#ab1234', C = '#3412ab';
const id = `e2e_${run}`;
(async () => {
  await post({ type: 'addNode', node: { id, name: `Test ${run}`, type: 'frame', parentId: null, x: 0, y: 0, w: 300, h: 200, fill: A } });
  browser = await chromium.launch({ args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1 });
  page.on('pageerror', (e) => { pageErrors.push(e.message); console.log('[pageerror]', e.message); });
  await page.goto(react ? `${web}/?react=feedback,layers` : web); // MELDFRAME_UI=react: the React copies of web-feedback (the banners, the toast, the empty state) and of the Layers panel (S5)
  await page.evaluate(() => { window.__stay = true; });

  await check('the page connects and shows the file that was saved before it opened', async () => {
    await until('connected', async () => (await state(page)) === 'live');
    await until('the saved frame is drawn', async () => (await count(page, A)) > 300);
  });
  if (react) {
    // A silent fallback to Svelte would pass every check below: ask that React drew the Layers panel (React puts a fibre on every node it makes).
    await check('the Layers panel is drawn by React', async () => {
      await until('the panel is drawn', async () => (await page.locator('[data-testid=layers]').count()) === 1);
      assert.equal(await page.evaluate(() => Object.keys(document.querySelector('[data-testid=layers]') ?? {}).some((k) => k.startsWith('__reactFiber$'))), true);
    });
  }
  await check('a change of an agent appears without a reload', async () => {
    await post({ type: 'updateNode', id, patch: { fill: B } });
    await until('the new fill', async () => (await count(page, B)) > 300);
    assert.ok((await count(page, A)) < 50, 'the old fill is gone');
    assert.equal(await page.evaluate(() => window.__stay), true, 'the page was not reloaded');
  });
  await check('a click on the frame selects it (the canvas draws the selection)', async () => {
    const before = await page.screenshot();
    // The frame is the centre of the screen after the view fitted the file: click there.
    const at = await page.evaluate(async () => {
      const c = document.querySelector('canvas');
      const r = c.getBoundingClientRect();
      return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
    });
    await page.mouse.click(at.x, at.y);
    await sleep(400);
    const after = await page.screenshot();
    assert.notDeepEqual(after, before, 'the selection changed what is drawn');
  });
  // Editing: real gestures; the server is asked what happened. The frame is fitted to the window, so its box on the screen
  // is found from its colour.
  if (results.every(Boolean)) {
    let box, z, start;
    await check('dragging the frame moves it by what the pointer moved, and one undo takes it back', async () => {
      box = await bbox(page, B); z = await zoomOf(page); start = await nodeOf(id);
      const c = { x: (box.x0 + box.x1) / 2, y: (box.y0 + box.y1) / 2 };
      await drag(page, c, { x: c.x + 80, y: c.y + 40 });
      const moved = await until('the move on the server', async () => { const n = await nodeOf(id); return n.x !== start.x ? n : null; });
      nearZoom(moved.x - start.x, 80, z, 2, 'moved x'); nearZoom(moved.y - start.y, 40, z, 2, 'moved y');
      // The readout's rounding is wider than the old tolerance, so the proportions, which do not depend on the zoom, are checked too.
      near((moved.x - start.x) / (moved.y - start.y), 2, 0.02, 'the move keeps the proportion of the pointer (80 by 40)');
      await page.keyboard.press('Control+z');
      await until('the move undone', async () => (await nodeOf(id)).x === start.x);
    });
    await check('resizing from the top-left handle is one change: one undo restores the origin and the size together', async () => {
      await page.waitForTimeout(300);
      box = await bbox(page, B); z = await zoomOf(page); start = await nodeOf(id);
      await drag(page, { x: box.x0 + 1, y: box.y0 + 1 }, { x: box.x0 + 31, y: box.y0 + 21 });
      const n = await until('the resize on the server', async () => { const r = await nodeOf(id); return r.w !== start.w ? r : null; });
      nearZoom(n.x - start.x, 30, z, 2, 'x'); nearZoom(n.y - start.y, 20, z, 2, 'y'); nearZoom(start.w - n.w, 30, z, 2, 'w'); nearZoom(start.h - n.h, 20, z, 2, 'h');
      // From the top-left handle the origin and the size move together by the same distance, whatever the zoom is.
      near(n.x - start.x, start.w - n.w, 0.01, 'x and w move together'); near(n.y - start.y, start.h - n.h, 0.01, 'y and h move together');
      await page.keyboard.press('Control+z');
      await until('one undo', async () => { const r = await nodeOf(id); return r.x === start.x && r.y === start.y && r.w === start.w && r.h === start.h; });
    });
    await check('R then a dragged box makes one node inside the frame, of that size; one undo removes it', async () => {
      await page.waitForTimeout(300);
      box = await bbox(page, B); z = await zoomOf(page);
      const before = (await nodesOnServer()).length;
      await page.keyboard.press('r');
      assert.equal(await page.getAttribute('[data-testid=tool]', 'data-tool'), 'rectangle');
      const c = { x: (box.x0 + box.x1) / 2, y: (box.y0 + box.y1) / 2 };
      await drag(page, { x: c.x - 60, y: c.y - 30 }, { x: c.x + 20, y: c.y + 20 });
      const all = await until('the new node', async () => { const a = await nodesOnServer(); return a.length === before + 1 ? a : null; });
      const made = all.find((x) => x.parentId === id);
      assert.ok(made, 'the node is in the frame');
      nearZoom(made.w, 80, z, 2, 'w'); nearZoom(made.h, 50, z, 2, 'h');
      assert.equal(await page.getAttribute('[data-testid=tool]', 'data-tool'), 'select', 'the tool goes back to select');
      await page.keyboard.press('Control+z');
      await until('the node removed by one undo', async () => (await nodesOnServer()).length === before);
    });
    // (Deleting a frame with content is a call the server asks about: no approval path before M2, so it is refused. A
    // small node is deleted.)
    await check('Delete removes a selected small node and undo brings it back', async () => {
      const leaf = `leaf_${run}`;
      await post({ type: 'addNode', node: { id: leaf, name: 'Leaf', type: 'rect', parentId: id, x: 10, y: 10, w: 60, h: 40, fill: '#ffaa00' } });
      await until('the leaf is drawn', async () => (await bbox(page, '#ffaa00')) !== null);
      const lb = await bbox(page, '#ffaa00');
      await page.mouse.click((lb.x0 + lb.x1) / 2, (lb.y0 + lb.y1) / 2);
      await page.keyboard.press('Delete');
      await until('the leaf deleted', async () => !(await nodeOf(leaf)));
      await page.keyboard.press('Control+z');
      await until('the leaf back', async () => Boolean(await nodeOf(leaf)));
    });
    await check('a double click on a text node opens the editor; Ctrl+Enter commits one change, one undo takes it back; Escape cancels', async () => {
      const tid = `text_${run}`;
      await post({ type: 'addNode', node: { id: tid, name: 'Words', type: 'text', parentId: id, x: 20, y: 20, w: 200, h: 40, text: 'Hello', fontSize: 24, fill: '#ffffff' } });
      await until('the text node is on the server', async () => Boolean(await nodeOf(tid)));
      await page.waitForTimeout(500);
      box = await bbox(page, B); z = await zoomOf(page);
      const at = { x: box.x0 + 30 * z, y: box.y0 + 30 * z };
      await page.mouse.dblclick(at.x, at.y);
      const field = page.locator('[data-testid=text-editor]');
      await field.waitFor({ state: 'visible', timeout: 5000 });
      assert.equal(await field.inputValue(), 'Hello');
      await field.fill('Hello world');
      await page.keyboard.press('Control+Enter');
      await until('the new text on the server', async () => (await nodeOf(tid)).text === 'Hello world');
      assert.equal(await field.count(), 0, 'the editor is closed');
      await page.keyboard.press('Control+z');
      await until('one undo', async () => (await nodeOf(tid)).text === 'Hello');
      await page.mouse.dblclick(at.x, at.y);
      await field.waitFor({ state: 'visible', timeout: 5000 });
      await field.fill('Never saved');
      await page.keyboard.press('Escape');
      await sleep(500);
      assert.equal((await nodeOf(tid)).text, 'Hello', 'Escape made no change');
    });
  }
  if (results.every(Boolean)) {
    await check('the layers panel lists the nodes; a click on a row selects it and the properties panel shows it', async () => {
      const row = page.locator(`[data-testid=layer][data-id="${id}"]`);
      await row.waitFor({ state: 'visible', timeout: 5000 });
      await row.locator('button').last().click();
      assert.equal(await row.getAttribute('data-selected'), 'true');
      assert.equal(await page.locator('[data-testid=prop-name]').inputValue(), `Test ${run}`);
    });
    await check('renaming in the properties panel is one change, and undo takes it back', async () => {
      const field = page.locator('[data-testid=prop-name]');
      await field.fill(`Renamed ${run}`);
      await field.press('Enter');
      await until('the new name on the server', async () => (await nodeOf(id)).name === `Renamed ${run}`);
      await page.keyboard.press('Control+z');
      await until('the name back', async () => (await nodeOf(id)).name === `Test ${run}`);
    });
    await check('a value that is not valid is put back and makes no change', async () => {
      const field = page.locator('[data-testid=prop-fill]');
      const before = await field.inputValue();
      await field.fill('not-a-colour');
      await field.press('Enter');
      await until('the field shows the document value again', async () => (await field.inputValue()) === before);
      assert.equal((await nodeOf(id)).fill, before);
    });
    await check('opacity is typed as a percentage, reaches the server as 0 to 1, and a value out of range is put back', async () => {
      const field = page.locator('[data-testid=prop-opacity]');
      await field.fill('40');
      await field.press('Enter');
      await until('the opacity on the server', async () => (await nodeOf(id)).opacity === 0.4);
      await field.fill('140');
      await field.press('Enter');
      await until('the field shows 40 again', async () => (await field.inputValue()) === '40');
      await page.keyboard.press('Control+z');
      await until('the opacity back', async () => [undefined, 1].includes((await nodeOf(id)).opacity));
    });
    await check('a tool button sets the tool', async () => {
      await page.click('[data-testid=tool-rectangle]');
      assert.equal(await page.getAttribute('[data-testid=tool]', 'data-tool'), 'rectangle');
      await page.click('[data-testid=tool-select]');
      assert.equal(await page.getAttribute('[data-testid=tool]', 'data-tool'), 'select');
    });
    await check('the buttons of the layer rows and the section toggles are at least 24 px', async () => {
      const small = await page.locator('[data-testid=layers] button, [data-testid=toggle-variables], [data-owns-keys] section > header button').evaluateAll((bs) => bs.filter((b) => { const r = b.getBoundingClientRect(); return r.width < 24 || r.height < 24; }).map((b) => b.outerHTML.slice(0, 90)));
      assert.deepEqual(small, []);
    });
    await check('plain keys stay with what has focus in a card or a menu, and still work from the canvas', async () => {
      const tool = () => page.getAttribute('[data-testid=tool]', 'data-tool');
      // a button of a card: a tool letter is not a command
      await page.locator('[data-testid=toggle-variables]').focus();
      await page.keyboard.press('r');
      assert.equal(await tool(), 'select', 'a tool letter typed on a card button changed the tool');
      // a layer row with its frame selected: Delete does not delete it
      await page.locator(`[data-testid=layer][data-id="${id}"] button`).last().click();
      await page.keyboard.press('Delete');
      await page.waitForTimeout(600);
      assert.equal(await page.locator('[data-testid=notice]').count(), 0, 'Delete typed on a layer row reached the editor');
      assert.ok(await nodeOf(id), 'the frame was deleted from a layer row');
      // an open menu: typing is its search, `?` is not the hint
      await page.click('[data-testid=frame-presets]');
      await page.locator('[role=menu]').waitFor();
      await page.keyboard.press('r');
      await page.keyboard.press('Shift+Slash');
      assert.equal(await tool(), 'select', 'typing in a menu changed the tool');
      assert.equal(await page.locator('[data-testid=shortcut-hint]').count(), 0, 'typing in a menu opened the hint');
      await page.keyboard.press('Escape');
      // from the page itself the letter is a command
      await page.evaluate(() => document.activeElement?.blur());
      await page.keyboard.press('r');
      assert.equal(await tool(), 'rectangle');
      await page.click('[data-testid=tool-select]');
    });
    await check('Enter on a focused tool button activates the button and does not start a text edit, with one text node selected', async () => {
      await page.locator(`[data-testid=layer][data-id="text_${run}"] button`).last().click();
      await page.locator('[data-testid=tool-rectangle]').focus();
      await page.keyboard.press('Enter');
      assert.equal(await page.getAttribute('[data-testid=tool]', 'data-tool'), 'rectangle', 'the button was not activated');
      assert.equal(await page.locator('[data-testid=text-editor]').count(), 0, 'Enter on the button started a text edit');
      await page.click('[data-testid=tool-select]');
      // from the page itself Enter still edits the selected text
      await page.evaluate(() => document.activeElement?.blur());
      await page.keyboard.press('Enter');
      await page.locator('[data-testid=text-editor]').waitFor({ timeout: 4000 });
      await page.keyboard.press('Escape');
    });
    await check('a delete the server refuses shows a notice', async () => {
      const row = page.locator(`[data-testid=layer][data-id="${id}"]`);
      await row.locator('button').last().click();
      await page.evaluate(() => document.activeElement?.blur()); // from the page: Delete on a row button belongs to the row
      await page.keyboard.press('Delete');
      await page.locator('[data-testid=notice]').waitFor({ state: 'visible', timeout: 8000 });
      assert.ok(await nodeOf(id), 'the frame is still on the server');
      // Another action does not keep the notice on screen: it goes away by itself.
      await page.click('[data-testid=tool-rectangle]');
      await page.click('[data-testid=tool-select]');
      await page.locator('[data-testid=notice]').waitFor({ state: 'detached', timeout: 9000 });
    });
  }
  // Needs a second actor: start the server with a second token (MELDFRAME_DEV_TOKENS="<dev token>:ws_dev:agent:dev,<token 2>:ws_dev:agent:claude")
  // and run this with MELDFRAME_E2E_TOKEN2=<token 2>.
  if (results.every(Boolean) && process.env.MELDFRAME_E2E_TOKEN2) {
    await check('a change by another actor appears at once and its row in the layers panel carries the actor', async () => {
      const other = `other_${run}`;
      const res = await fetch(`${server}/v1/files/${file}/tools/apply_commands`, {
        method: 'POST',
        headers: { 'content-type': 'application/json', authorization: `Bearer ${process.env.MELDFRAME_E2E_TOKEN2}` },
        body: JSON.stringify({ ops: [{ key: `e2e2-${run}`, command: { type: 'addNode', node: { id: other, name: 'From claude', type: 'rect', parentId: id, x: 5, y: 5, w: 40, h: 30, fill: '#00aa88' } }, deps: [] }] }),
      });
      assert.equal(res.status, 200, `op refused: ${res.status}`);
      const chip = page.locator(`[data-testid=layer][data-id="${other}"] [data-testid=actor]`);
      await chip.waitFor({ state: 'visible', timeout: 8000 });
      assert.equal((await chip.textContent())?.trim(), 'claude');
      // Its own changes carry no label.
      assert.equal(await page.locator(`[data-testid=layer][data-id="${id}"] [data-testid=actor]`).count(), 0);
    });
  }
  // An agent asks the open editor for a PNG through MCP (second token, as above).
  if (results.every(Boolean) && process.env.MELDFRAME_E2E_TOKEN2) {
    await check('an agent captures the frame through MCP and gets a PNG of the right size from this editor', async () => {
      const frame = await nodeOf(id);
      const res = await fetch(`${server}/mcp`, {
        method: 'POST',
        headers: { 'content-type': 'application/json', accept: 'application/json, text/event-stream', authorization: `Bearer ${process.env.MELDFRAME_E2E_TOKEN2}` },
        body: JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'tools/call', params: { name: 'capture', arguments: { file, nodeId: id, scale: 1 } } }),
      });
      const part = (await res.json()).result?.content?.[0];
      assert.equal(part?.type, 'image', `not an image: ${JSON.stringify(part)?.slice(0, 200)}`);
      assert.equal(part.mimeType, 'image/png');
      const png = Buffer.from(part.data, 'base64');
      assert.equal(png.subarray(1, 4).toString(), 'PNG');
      near(png.readUInt32BE(16), frame.w, 1, 'width');
      near(png.readUInt32BE(20), frame.h, 1, 'height');
    });
    await check('a capture of a node that is not there is refused with node_not_found', async () => {
      const res = await fetch(`${server}/mcp`, {
        method: 'POST',
        headers: { 'content-type': 'application/json', accept: 'application/json, text/event-stream', authorization: `Bearer ${process.env.MELDFRAME_E2E_TOKEN2}` },
        body: JSON.stringify({ jsonrpc: '2.0', id: 2, method: 'tools/call', params: { name: 'capture', arguments: { file, nodeId: 'no_such_node', scale: 1 } } }),
      });
      assert.match(JSON.stringify(await res.json()), /node_not_found/);
    });
  }
  // The reconnect check stops the development server: only when the checks before it passed (otherwise leave it).
  if (results.every(Boolean)) await check('the server goes away: the page says so; it comes back: the page reconnects and shows what was done meanwhile', async () => {
    // Stop only what listens on the server's port (not every process that has the name).
    const port = new URL(server).port || '80';
    const pids = spawnSync('lsof', ['-ti', `tcp:${port}`, '-sTCP:LISTEN']).stdout.toString().split(/\s+/).filter(Boolean);
    assert.ok(pids.length, `nothing listens on port ${port}`);
    for (const pid of pids) process.kill(Number(pid));
    await until('not live', async () => (await state(page)) !== 'live', 20000);
    // The restart keeps this process's environment (CARGO_TARGET_DIR included) and logs to a file.
    const log = openSync('/tmp/editor-e2e-dev.log', 'w');
    const dev = spawn('bash', ['scripts/dev.sh'], { cwd: root, detached: true, stdio: ['ignore', log, log] });
    dev.unref();
    closeSync(log);
    await until('the server answers', async () => { try { return (await fetch(`${server}/v1/health`)).ok; } catch { return false; } }, 120000);
    await post({ type: 'updateNode', id, patch: { fill: C } });
    await until('live again', async () => (await state(page)) === 'live', 90000);
    await until('the change made while it reconnected', async () => (await count(page, C)) > 300, 30000);
    assert.equal(await page.evaluate(() => window.__stay), true, 'the page was not reloaded');
  });
  // Image fills (E2.5): an image the server holds is drawn in the box of its node by its fit; one it does not hold is said
  // once, with its code, and the node shows its fill. Before the preset check below: that frame is large and lies over the others.
  if (results.every(Boolean)) {
    const S = ['#e91e63', '#00bcd4', '#8bc34a']; // three stripes: top, middle, bottom
    const F = '#204060'; // the fill under the image, seen where the image does not reach
    await check('an image fill is drawn by its fit: cover crops, contain leaves the fill beside it, fill stretches', async () => {
      const asset = await upload(pngOf(20, 20, (y) => (y < 7 ? S[0] : y < 14 ? S[1] : S[2])));
      const fits = ['fill', 'contain', 'cover'];
      const aspect = {};
      for (const fit of fits) {
        // One node at a time, so the stripes of each can be found by their colours.
        const nid = `${id}_img_${fit}`;
        await post({ type: 'addNode', node: { id: nid, name: `Image ${fit}`, type: 'rect', parentId: id, x: 10, y: 10, w: 120, h: 60, fill: F, image: { assetId: asset, fit } } });
        const top = await until(`the ${fit} image is drawn`, async () => bbox(page, S[0]));
        const mid = await until(`the middle stripe of the ${fit} image`, async () => bbox(page, S[1]));
        aspect[fit] = { w: top.x1 - top.x0 + 1, h: top.y1 - top.y0 + 1, mid: mid.y1 - mid.y0 + 1, fill: await count(page, F) };
        await post({ type: 'removeNode', id: nid });
        await until(`the ${fit} image is gone`, async () => (await bbox(page, S[0])) === null);
      }
      // The top stripe of the image, as a box: stretched over the whole width, a stripe a third of the height (fill); a
      // half-width stripe (contain: the image is a square in a box twice as wide); a wide, thin sliver (cover: the box
      // shows the middle of the image, so the top stripe is cut to its last rows).
      const ratio = (f) => aspect[f].w / aspect[f].h;
      assert.ok(ratio('contain') > 2 && ratio('contain') < 4, `contain: ${JSON.stringify(aspect.contain)}`);
      assert.ok(ratio('fill') > 4.5 && ratio('fill') < 7.5, `fill: ${JSON.stringify(aspect.fill)}`);
      assert.ok(ratio('cover') > 8, `cover: ${JSON.stringify(aspect.cover)}`);
      assert.ok(aspect.contain.fill > 50, 'contain leaves the fill of the node beside the image');
    });
    await check('an image the server does not hold is reported as asset_not_found and the node shows its fill', async () => {
      const nid = `${id}_img_missing`;
      await post({ type: 'addNode', node: { id: nid, name: 'Image missing', type: 'rect', parentId: id, x: 10, y: 80, w: 120, h: 60, fill: '#a0522d', image: { assetId: 'f'.repeat(64), fit: 'cover' } } });
      const notice = page.locator('[data-testid=notice]');
      await notice.waitFor({ state: 'visible', timeout: 8000 });
      assert.match((await notice.textContent()) ?? '', /asset_not_found/);
      assert.ok((await count(page, '#a0522d')) > 100, 'the fill shows instead of the image');
    });
  }
  // Last among the edits: the new frame is large and lies over the others.
  if (results.every(Boolean)) {
    await check('a frame preset makes a top-level frame of its size and name, and selects it', async () => {
      const known = new Set((await nodesOnServer()).map((x) => x.id));
      const before = known.size;
      await page.click('[data-testid=frame-presets]'); // the presets are in a menu
      await page.click('[data-testid=preset-mobile]');
      const all = await until('the preset frame on the server', async () => { const a = await nodesOnServer(); return a.length === before + 1 ? a : null; });
      const made = all.find((x) => !known.has(x.id));
      assert.equal(made.name, 'Mobile');
      assert.equal(made.parentId, null);
      assert.equal(`${made.w}x${made.h}`, '390x844');
      await until('the new frame is selected', async () => (await page.locator(`[data-testid=layer][data-id="${made.id}"]`).getAttribute('data-selected')) === 'true');
      // (Undoing the creation of a frame sends a delete of a frame, which is Ask: the server answers approval_required and the delete waits for an owner who cannot answer yet, so it is not done.)
    });
    await check('the Variant button copies the selected frame beside it, names it, and selects the copy', async () => {
      const known = new Set((await nodesOnServer()).map((x) => x.id));
      const source = await page.locator('[data-testid=layer][data-selected=true]').first().getAttribute('data-id');
      assert.ok(source, 'a frame is selected');
      const from = (await nodesOnServer()).find((x) => x.id === source);
      await page.click('[data-testid=file-menu]'); // the variant is in the file menu
      assert.equal(await page.locator('[data-testid=make-variant]').isDisabled(), false);
      await page.click('[data-testid=make-variant]');
      const all = await until('the variant on the server', async () => { const a = await nodesOnServer(); return a.length === known.size + 1 ? a : null; });
      const copy = all.find((x) => !known.has(x.id));
      assert.equal(copy.name, `${from.name} variant`);
      assert.equal(copy.parentId, from.parentId);
      assert.ok(copy.x >= from.x + from.w + 40 - 0.5, `the copy lies right of the original: ${copy.x} vs ${from.x + from.w}`);
      assert.equal(copy.y, from.y);
      await until('the copy is selected', async () => (await page.locator(`[data-testid=layer][data-id="${copy.id}"]`).getAttribute('data-selected')) === 'true');
    });
  }
  await check('no uncaught error in the page', async () => assert.deepEqual(pageErrors, []));
  await browser.close();
  const failed = results.filter((r) => !r).length;
  console.log(failed ? `${failed} of ${results.length} checks failed` : `all ${results.length} checks passed`);
  process.exit(failed ? 1 : 0);
})().catch(async (e) => {
  console.error(`editor-e2e: ${e.message}`);
  await browser?.close().catch(() => {});
  process.exit(1);
});
