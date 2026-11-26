// Browser check of the Remove button of the Border section against a real server (issue 267, decision 40):
//   MELDFRAME_WEB_FILE=<a new file name> node packages/web-panels/scripts/border-remove-e2e.mjs <web-url> <server-url>
// The Border section of the Properties panel has a Remove border button while the border width is above 0 and none without a
// border. The check selects a node with no border (no button, no position control), a node with a border (the button, its name,
// its hit size, its place after the position control, a border drawn on the canvas), clicks it and checks that exactly one op
// goes out to the server, that it is the command the width field and an agent send (updateNode with the border at width 0,
// colour and position as they were), that the server holds it, that the canvas draws no border and that focus is on the width
// field, which stays; that one undo brings width, colour and position back (server, panel and canvas); that Enter on the focused
// button removes the border too and focus lands on the width field again; that typing 0 in the width field sends the same op; and,
// last, that an agent's apply_commands with the command of the button, aimed at a twin node, gives the same node as typing 0.
// Start the server and the page as the header of packages/web-panels/scripts/image-fill-e2e.mjs says: a file name that is new for
// every run (the check refuses a file that has content), the server on a free port, the development token of the page in
// .dev-token at the root and in the server's token list, the page's URL and the server's URL as the two arguments, no defaults.
// The WASM bridge must be built from this revision (`just wasm-build`), and Playwright comes from the global module path. Both URLs
// must be loopback (the token is sent there). It adds a frame and four nodes to the file and leaves them: development server only.
// The checks run in order and share the file; one that fails can fail the ones after it.
// MELDFRAME_UI=react opens the page with `/?react=properties` (the React copy of the Properties panel, E7.3 slice S6) and first asks that React drew it, so that a
// silent fallback to Svelte cannot pass; without it the Svelte copy runs and the same checks apply.
// MELDFRAME_E2E_SHOTS=<dir> saves screenshots (and one of the page when a check fails).
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';

const { chromium } = createRequire(import.meta.url)('playwright');
const [web, server] = process.argv.slice(2);
const react = process.env.MELDFRAME_UI === 'react';
const root = new URL('../../../', import.meta.url).pathname;
const file = process.env.MELDFRAME_WEB_FILE;
const shots = process.env.MELDFRAME_E2E_SHOTS;
if (!web || !server || !file) throw new Error('usage: MELDFRAME_WEB_FILE=<a new file name> node border-remove-e2e.mjs <web-url> <server-url> (see the top of this file; no defaults)');
for (const u of [web, server]) {
  if (!['127.0.0.1', 'localhost', '[::1]'].includes(new URL(u).hostname)) throw new Error(`refusing to send the token to ${u}: the web and server URLs must be loopback (127.0.0.1, localhost or ::1)`);
}
const token = readFileSync(`${root}.dev-token`, 'utf8').trim();
const auth = { authorization: `Bearer ${token}` };
const run = Math.random().toString(36).slice(2, 7);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const pageErrors = [];
let browser;
let page;
let n = 0;

async function post(command) {
  const res = await fetch(`${server}/v1/files/${file}/tools/apply_commands`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', ...auth },
    body: JSON.stringify({ ops: [{ key: `br-${run}-${++n}`, command, deps: [] }] }),
  });
  assert.equal(res.status, 200, `op refused: ${res.status} ${await res.text()}`);
}
// A file the server does not know yet is 404 and empty; any other answer that is not the file is an error (the guard cannot pass on it).
const snapshot = async () => {
  const res = await fetch(`${server}/v1/files/${file}`, { headers: auth });
  if (res.status === 200) return res.json();
  if (res.status === 404) return { seq: 0, nodes: [] };
  throw new Error(`reading the file: ${res.status} ${await res.text()}`);
};
const nodeOf = async (id) => (await snapshot()).nodes.find((x) => x.id === id);
async function until(what, fn, ms = 15000) {
  const t0 = Date.now();
  for (;;) {
    const v = await fn();
    if (v) return v;
    if (Date.now() - t0 > ms) throw new Error(`timed out waiting for: ${what}`);
    await sleep(150);
  }
}

/**
 * The ink of one colour on the canvas: how many pixels are nearly `want` (a 3-number colour, tolerance `tol`) and the left and right
 * edge of them. Only the canvas element is read (it sits between the layers panel and the right panel, so the panels are out of it: the Fill
 * swatch of the right panel has the colour of the text), and its box is read from the page on every call.
 */
async function ink(want, tol = 60) {
  const box = await page.locator('canvas').first().boundingBox();
  assert.ok(box, 'the canvas has a box');
  const b64 = (await page.screenshot()).toString('base64');
  return page.evaluate(async ({ b64, want, tol, box }) => {
    const img = new Image();
    img.src = `data:image/png;base64,${b64}`;
    await img.decode();
    const c = document.createElement('canvas');
    c.width = img.width; c.height = img.height;
    const g = c.getContext('2d');
    g.drawImage(img, 0, 0);
    const d = g.getImageData(0, 0, c.width, c.height).data;
    let count = 0, min = Infinity, max = -Infinity;
    for (let i = 0; i < d.length; i += 4) {
      const x = (i / 4) % c.width;
      const y = Math.floor(i / 4 / c.width);
      if (x < box.x || x >= box.x + box.width || y < box.y || y >= box.y + box.height) continue;
      if (Math.abs(d[i] - want[0]) < tol && Math.abs(d[i + 1] - want[1]) < tol && Math.abs(d[i + 2] - want[2]) < tol) {
        count++;
        if (x < min) min = x;
        if (x > max) max = x;
      }
    }
    return { count, min, max };
  }, { b64, want, tol, box });
}
const MAGENTA = [255, 0, 255]; // the border
const BORDER = { width: 6, color: '#ff00ff', position: 'outside' };
// The twin and the typed node have another colour, so that the ink of MAGENTA on the canvas is the border of the node under test.
const OTHER = { width: 4, color: '#ff8000', position: 'center' };

const $ = (sel) => page.locator(sel);
const results = [];
async function check(name, fn) {
  try {
    await fn();
    results.push(true);
    console.log(`ok   ${name}`);
  } catch (e) {
    results.push(false);
    if (shots && page) await page.screenshot({ path: `${shots}/border-remove-fail-${results.length}.png` }).catch(() => {});
    console.log(`FAIL ${name}: ${String(e.message ?? e).split('\n').slice(0, process.env.MELDFRAME_E2E_DEBUG ? 12 : 1).join(' | ')}`);
  }
}
const shot = (name) => (shots && page ? page.screenshot({ path: `${shots}/border-remove-${name}.png` }) : null);

const frame = `frame_${run}`;
const plain = `plain_${run}`;
const boxed = `boxed_${run}`;
const twin = `twin_${run}`;
const typed = `typed_${run}`;
// The panel shows the node once its name is in the name field (the panel of the node selected before stays up until then).
const select = async (id, name) => {
  await $(`[data-testid=layer][data-id="${id}"] button`).last().click();
  await until('the properties panel shows the node', async () => (await $('[data-testid=prop-name]').inputValue().catch(() => '')) === name);
};
const undo = async () => {
  await page.locator('[data-testid=undo]').click();
};
let remove; // the locators, made once the page exists
let width;
const focusIsWidth = () => width.evaluate((e) => document.activeElement === e);
// What the page sends to the server: every request that carries ops, as the commands it holds.
const sent = [];
const commandsSent = () => sent.flatMap((m) => (Array.isArray(m.ops) ? m.ops.map((o) => o.command) : []));

(async () => {
  assert.equal((await snapshot()).seq, 0, 'the file has content already: start the page and the check with a file name that is new');
  await post({ type: 'addNode', node: { id: frame, name: `Card ${run}`, type: 'frame', parentId: null, x: 0, y: 0, w: 400, h: 300, fill: '#ffffff' } });
  const rect = (id, name, x, y, border) => ({ type: 'addNode', node: { id, name: `${name} ${run}`, type: 'rect', parentId: frame, x, y, w: 80, h: 60, fill: '#00cc00', ...(border ? { border } : {}) } });
  await post(rect(plain, 'Plain', 20, 20));
  await post(rect(boxed, 'Boxed', 140, 20, BORDER));
  await post(rect(twin, 'Twin', 260, 20, OTHER));
  await post(rect(typed, 'Typed', 20, 140, OTHER));
  browser = await chromium.launch({ args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
  page = await browser.newPage({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1 });
  remove = $('[data-testid=prop-border-remove]');
  width = $('[data-testid=prop-border-width]');
  page.on('pageerror', (e) => { pageErrors.push(e.message); console.log('[pageerror]', e.message); });
  // The page sends its ops with POST /v1/files/{file}/tools/apply_commands, the tool an agent calls.
  page.on('request', (r) => {
    if (r.method() !== 'POST' || !r.url().includes('/tools/apply_commands')) return;
    try { sent.push(JSON.parse(r.postData() ?? '')); } catch { /* not JSON */ }
  });
  await page.goto(react ? `${web.replace(/\/?$/, '/')}?react=properties` : web);
  await until('the editor is live', async () => (await page.getAttribute('[data-testid=connection]', 'data-state')) === 'live');
  await until('the nodes are listed', async () => (await $(`[data-testid=layer][data-id="${typed}"]`).count()) === 1);
  // MELDFRAME_UI=react: the page must really be the React copy of the Properties panel (a fallback to Svelte would pass every other check).
  if (react) await page.waitForSelector('[data-testid=properties]', { timeout: 20_000 }); // the React chunk loads on demand: the panel is there a moment after the editor is live
  if (react) assert.equal(await page.evaluate(() => Object.keys(document.querySelector('[data-testid=properties]') ?? {}).some((k) => k.startsWith('__reactFiber$'))), true, 'the Properties panel has no React fibre: the page fell back to Svelte');
  let drawn;
  let buttonCommand; // the command the click sent, for the agent's check at the end

  await check('a node with no border has no Remove button and no position control', async () => {
    await select(plain, `Plain ${run}`);
    assert.equal(await width.inputValue(), '0');
    assert.equal(await remove.count(), 0, 'no button without a border');
    assert.equal(await $('[data-testid=prop-border-position]').count(), 0);
    assert.equal(await page.getByRole('button', { name: 'Remove border', exact: true }).count(), 0);
  });

  await check('a node with a border has the button: named Remove border, 24 px or more, after the position control, and the canvas draws the border', async () => {
    await select(boxed, `Boxed ${run}`);
    assert.equal(await width.inputValue(), '6');
    assert.equal(await remove.count(), 1);
    assert.equal(await page.getByRole('button', { name: 'Remove border', exact: true }).count(), 1, 'one button by its accessible name');
    const box = await remove.boundingBox();
    assert.ok(box && box.width >= 24 && box.height >= 24, `a hit target of at least 24 px: ${JSON.stringify(box)}`);
    const after = await $('[data-testid=prop-border-position]').evaluate((e, b) => !!(e.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING), await remove.elementHandle());
    assert.equal(after, true, 'the button follows the position control in the focus order');
    drawn = await ink(MAGENTA);
    assert.ok(drawn.count > 50, `the canvas draws the border (${drawn.count} pixels)`);
    await shot('with-border');
  });

  await check('the button removes the border: one op, the command of the width field and of an agent; the server, the panel and the canvas agree; focus is on the width field', async () => {
    const from = commandsSent().length;
    await remove.click();
    await until('width 0 on the server', async () => (await nodeOf(boxed))?.border?.width === 0);
    const went = commandsSent().slice(from);
    assert.deepEqual(went, [{ type: 'updateNode', id: boxed, patch: { border: { width: 0, color: '#ff00ff', position: 'outside' } } }], 'exactly one op, and only the width changed');
    buttonCommand = went[0];
    assert.deepEqual((await nodeOf(boxed)).border, { width: 0, color: '#ff00ff', position: 'outside' }, 'colour and position stay');
    await until('the button and the position control are gone', async () => (await remove.count()) === 0 && (await $('[data-testid=prop-border-position]').count()) === 0);
    assert.equal(await width.inputValue(), '0');
    assert.equal(await focusIsWidth(), true, 'focus is on the width field, not on the page');
    await until('the canvas draws no border', async () => (await ink(MAGENTA)).count < drawn.count * 0.05);
    await shot('removed');
  });

  await check('one undo brings width, colour and position back, on the server, in the panel and on the canvas', async () => {
    await undo();
    await until('the border is back on the server', async () => { const b = (await nodeOf(boxed))?.border; return b?.width === 6 && b.color === '#ff00ff' && b.position === 'outside'; });
    await until('the button is back', async () => (await remove.count()) === 1);
    assert.equal(await width.inputValue(), '6');
    assert.equal(await $('[data-testid=prop-border-color]').inputValue(), '#ff00ff');
    assert.equal(await $('[data-testid=prop-border-position] [role=radio][aria-checked=true]').getAttribute('title'), 'Outside');
    await until('the canvas is as it was', async () => { const i = await ink(MAGENTA); return Math.abs(i.count - drawn.count) <= drawn.count * 0.02 && i.min === drawn.min; });
    await shot('undone');
  });

  await check('the keyboard works the button: Enter on it removes the border, focus lands on the width field, and one undo brings it back', async () => {
    await remove.focus();
    await page.keyboard.press('Enter');
    await until('width 0 on the server', async () => (await nodeOf(boxed))?.border?.width === 0);
    await until('the button is gone', async () => (await remove.count()) === 0);
    assert.equal(await focusIsWidth(), true, 'focus is on the width field');
    await undo();
    await until('the border is back on the server', async () => (await nodeOf(boxed))?.border?.width === 6);
    await until('the button is back', async () => (await remove.count()) === 1);
  });

  await check('typing 0 in the width field sends the same command as the button', async () => {
    await select(typed, `Typed ${run}`);
    const from = commandsSent().length;
    await width.fill('0');
    await page.keyboard.press('Enter');
    await until('width 0 on the server', async () => (await nodeOf(typed))?.border?.width === 0);
    assert.deepEqual(commandsSent().slice(from), [{ type: 'updateNode', id: typed, patch: { border: { width: 0, color: '#ff8000', position: 'center' } } }]);
    assert.equal(await remove.count(), 0, 'the button follows the width field to 0');
  });

  await check('an agent that applies the command of the button to another node with apply_commands gets the same node as typing 0 in the width field', async () => {
    assert.ok(buttonCommand, 'the command of the button was captured');
    assert.equal((await nodeOf(twin)).border.width, 4);
    // The agent's command is the button's own (same type, same keys, width 0), aimed at the twin and keeping the twin's colour and position.
    const agent = { ...buttonCommand, id: twin, patch: { border: { ...buttonCommand.patch.border, color: OTHER.color, position: OTHER.position } } };
    assert.deepEqual(Object.keys(agent.patch.border).sort(), ['color', 'position', 'width'], 'the button sends the border and nothing more');
    assert.deepEqual(Object.keys(agent.patch), ['border']);
    assert.equal(agent.patch.border.width, 0);
    await post(agent);
    const [byAgent, byTyping] = [await nodeOf(twin), await nodeOf(typed)];
    assert.deepEqual(byAgent.border, byTyping.border, 'the agent and the width field leave the same border');
    assert.deepEqual(byAgent.border, { width: 0, color: OTHER.color, position: OTHER.position });
    await until('the panel of the twin has no button', async () => { await select(twin, `Twin ${run}`); return (await remove.count()) === 0; });
  });

  const failed = results.filter((r) => r === false).length;
  console.log(`${results.filter((r) => r === true).length} passed, ${failed} failed`);
  console.log(pageErrors.length ? `${pageErrors.length} page error(s)` : 'no page errors');
  await browser.close();
  process.exit(failed || pageErrors.length ? 1 : 0);
})().catch(async (e) => {
  console.log('ERROR', e);
  await browser?.close().catch(() => {});
  process.exit(1);
});
