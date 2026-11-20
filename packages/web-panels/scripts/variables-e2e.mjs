// Browser check of the Variables panel against a real server: node packages/web-panels/scripts/variables-e2e.mjs [web-url] [server-url]
// The panel and the mode switch in a browser: a frame is bound to a variable, and switching the mode re-draws it on the canvas.
// Needs the development server (`just dev`, or the server binary with MELDFRAME_DEV_TOKENS and no DATABASE_URL, token in
// .dev-token at the root) and `just web` (Vite) running, the WASM bridge built, and Playwright from the global module
// path. It adds one frame, one collection and one variable to the file, and removes the variable and the collection at
// the end (the frame stays: deleting a frame needs the owner's approval): run it against the development server only.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';

const { chromium } = createRequire(import.meta.url)('playwright');
const web = process.argv[2] ?? 'http://localhost:5173';
const server = process.argv[3] ?? 'http://127.0.0.1:8080';
const root = new URL('../../../', import.meta.url).pathname;
const token = readFileSync(`${root}.dev-token`, 'utf8').trim();
const file = process.env.MELDFRAME_WEB_FILE ?? 'file_dev';
const run = Math.random().toString(36).slice(2, 8);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const auth = { authorization: `Bearer ${token}` };
const pageErrors = [];
let n = 0;
let browser;

async function post(command) {
  const res = await fetch(`${server}/v1/files/${file}/tools/apply_commands`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', ...auth },
    body: JSON.stringify({ ops: [{ key: `vp-${run}-${++n}`, command, deps: [] }] }),
  });
  assert.equal(res.status, 200, `op refused: ${res.status} ${await res.text()}`);
}
const snapshot = async () => (await fetch(`${server}/v1/files/${file}`, { headers: auth })).json();
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
/** Whether a colour fills an area of the canvas: the swatches of the panel are about 150 pixels of the same colours. */
const filled = async (page, hex) => (await count(page, hex)) > 1000;
async function until(what, fn, ms = 20000) {
  const t0 = Date.now();
  for (;;) {
    const v = await fn();
    if (v) return v;
    if (Date.now() - t0 > ms) throw new Error(`timed out waiting for: ${what}`);
    await sleep(200);
  }
}

const LIGHT = '#00aa55';
const DARK = '#aa0055';
// A colour of its own for every run, so a frame left by an earlier run cannot satisfy a check of this one.
const EDITED = `#${[0, 1, 2].map(() => (40 + Math.floor(Math.random() * 180)).toString(16)).join('')}`;
const col = `col_${run}`;
const bg = `bg_${run}`;
const frame = `frame_${run}`;
const results = [];
async function check(name, fn) {
  try { await fn(); results.push(true); console.log(`ok   ${name}`); } catch (e) { results.push(false); console.log(`FAIL ${name}: ${e.message}`); }
}

(async () => {
  await post({ type: 'addCollection', collection: { id: col, name: `appearance-${run}`, modes: [{ id: 'light', name: 'light' }, { id: 'dark', name: 'dark' }] } });
  await post({ type: 'addVariable', variable: { id: bg, collection: col, name: 'color/bg', type: 'color', values: { light: LIGHT, dark: DARK } } });
  await post({ type: 'addNode', node: { id: frame, name: `Card ${run}`, type: 'frame', parentId: null, x: 0, y: 0, w: 300, h: 200, fill: '#ffffff' } });
  browser = await chromium.launch({ args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1 });
  page.on('pageerror', (e) => { pageErrors.push(e.message); console.log('[pageerror]', e.message); });
  await page.goto(web);
  await until('the editor is live', async () => (await page.getAttribute('[data-testid=connection]', 'data-state')) === 'live');
  await until('the frame is listed', async () => (await page.locator(`[data-testid=layer][data-id="${frame}"]`).count()) === 1);

  await check('the panel is closed until the person opens it, then lists the collection and the variable', async () => {
    assert.equal(await page.locator('[data-testid=variables]').count(), 0);
    await page.click('[data-testid=toggle-variables]');
    await until('the row', async () => (await page.locator(`[data-testid=var-row][data-id="${bg}"]`).count()) === 1);
    assert.equal(await page.locator(`[data-testid=var-modes][data-id="${col}"] [role=radio]`).count(), 2);
    assert.equal(await page.inputValue(`[data-testid=var-row][data-id="${bg}"] [data-testid=var-value]`), LIGHT);
    // The list scrolls inside a capped box, so Properties stays on screen below it.
    const list = await page.locator('[data-testid=var-list]').boundingBox();
    assert.ok(list.height <= 192 + 1, `the list is capped: ${list.height}`);
    assert.ok(await page.locator('[data-testid=properties]').isVisible(), 'properties is on the page');
  });

  await check('binding the selected frame paints the variable, and the file holds the binding', async () => {
    assert.equal(await filled(page, LIGHT), false, 'the frame is white before the binding');
    await page.click(`[data-testid=layer][data-id="${frame}"] button >> nth=-1`);
    await page.click(`[data-testid=var-row][data-id="${bg}"] [data-testid=var-bind][data-field=fill]`);
    await until('the light colour on the canvas', () => filled(page, LIGHT));
    const node = await until('the binding on the server', async () => (await snapshot()).nodes.find((x) => x.id === frame && x.fill?.var === bg));
    assert.deepEqual(node.fill, { var: bg });
    assert.equal(await page.getAttribute(`[data-testid=var-row][data-id="${bg}"] [data-testid=var-bind][data-field=fill]`, 'aria-pressed'), 'true');
  });

  await check('switching to dark re-draws the frame and writes nothing to the file', async () => {
    const before = (await snapshot()).seq;
    await page.click(`[data-testid=var-modes][data-id="${col}"] [role=radio]:has-text("dark")`);
    await until('the dark colour on the canvas', () => filled(page, DARK));
    assert.equal(await filled(page, LIGHT), false, 'the light colour is gone');
    await sleep(500);
    assert.equal((await snapshot()).seq, before, 'no op was sent');
    await page.click(`[data-testid=var-modes][data-id="${col}"] [role=radio]:has-text("light")`);
    await until('the light colour again', () => filled(page, LIGHT));
  });

  await check('editing the value of a mode re-draws the frame and reaches the server', async () => {
    await page.click(`[data-testid=var-modes][data-id="${col}"] [role=radio]:has-text("dark")`);
    const field = page.locator(`[data-testid=var-row][data-id="${bg}"] [data-testid=var-value]`);
    await field.fill(EDITED);
    await field.press('Enter');
    await until('the edited colour on the canvas', () => filled(page, EDITED));
    const v = await until('the value on the server', async () => (await snapshot()).variables?.variables.find((x) => x.id === bg && x.values.dark === EDITED));
    assert.equal(v.values.light, LIGHT);
    await field.fill('not a colour');
    await field.press('Enter');
    await until('the field shows the stored value again', async () => (await field.inputValue()) === EDITED);
  });

  await check('the search narrows the list and says so', async () => {
    // An earlier run on the same file leaves its variables: the total is whatever the file holds.
    const total = (await page.textContent('[data-testid=var-count]')).trim().split(' of ')[1];
    await page.fill('[data-testid=var-search]', 'zzz');
    await until('no match', async () => (await page.textContent('[data-testid=var-count]')).trim() === `0 of ${total}`);
    assert.equal(await page.locator('[data-testid=var-row]').count(), 0);
    await page.fill('[data-testid=var-search]', 'BG');
    await until('the variable is listed', async () => (await page.locator(`[data-testid=var-row][data-id="${bg}"]`).count()) === 1);
    await page.fill('[data-testid=var-search]', '');
  });

  await check('unbinding keeps the colour on view as a literal', async () => {
    await page.click(`[data-testid=var-row][data-id="${bg}"] [data-testid=var-bind][data-field=fill]`);
    const node = await until('the literal on the server', async () => (await snapshot()).nodes.find((x) => x.id === frame && typeof x.fill === 'string' && x.fill !== '#ffffff'));
    assert.equal(node.fill, EDITED);
    await page.click(`[data-testid=var-modes][data-id="${col}"] [role=radio]:has-text("light")`);
    await sleep(300);
    assert.ok(await filled(page, EDITED), 'the literal does not follow the mode');
  });

  await check('no uncaught error in the page', async () => assert.deepEqual(pageErrors, []));
  await browser.close();
  // Leave no variable behind (it is removable once the frame lets go of it); a failed run may leave it, and the colours of the next run differ.
  for (const command of [{ type: 'removeVariable', id: bg }, { type: 'removeCollection', id: col }]) await post(command).catch(() => {});
  const failed = results.filter((r) => !r).length;
  console.log(failed ? `${failed} of ${results.length} checks failed` : `all ${results.length} checks passed`);
  process.exit(failed ? 1 : 0);
})().catch(async (e) => {
  console.error(`variables-e2e: ${e.message}`);
  await browser?.close().catch(() => {});
  process.exit(1);
});
