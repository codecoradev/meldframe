// Browser check of the Properties panel against a real server:
//   MELDFRAME_WEB_FILE=<a new file name> node packages/web-panels/scripts/properties-e2e.mjs <web-url> <server-url>
// Start the server and the page as the header of packages/web-panels/scripts/image-fill-e2e.mjs says: a file name that is new for every
// run (the check refuses a file that has content), the server on a free port (in memory, no DATABASE_URL), the development token of the
// page in .dev-token at the root and in the server's token list, the page's URL and the server's URL as the two arguments, no defaults.
// The WASM bridge must be built from this revision, and Playwright comes from the global module path. Both URLs must be loopback (the
// token is sent there). It adds a frame, a rectangle and a text to the file and leaves them: development server only. Hand-run, not in CI.
// What it checks: the fields send what is typed (the name, a position, an opacity in percent), a value the document does not take sends nothing
// and the field shows the document's value again, Escape puts the value back, a field is a new field for the next node (what was typed for
// one never reaches another), the sections follow the kind of node (the layout of a frame, the text section of a text), a section's
// own controls (the layout direction, Clip content, Add shadow and Remove shadow) change the node on the server and one undo
// brings it back, and the fields follow a change of an agent.
// MELDFRAME_UI=react opens the page with `/?react=properties` (the React copy of the panel, E7.3 slice S6) and a first check asks that React drew
// it, so that a silent fallback to Svelte cannot pass; without it the Svelte copy runs and the same checks apply. With React, a last check blocks
// the chunks of the React copy and asks that the Svelte panel is drawn and works, so that a chunk that does not load never leaves a page with no
// properties. The block matches the paths of the Vite dev server's modules (`.../react/PropertiesPanel.tsx`), so that check is for the dev
// server only and not for a production build (whose chunks have hashed names).
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';

const { chromium } = createRequire(import.meta.url)('playwright');
const react = process.env.MELDFRAME_UI === 'react';
const [web, server] = process.argv.slice(2);
const root = new URL('../../../', import.meta.url).pathname;
const file = process.env.MELDFRAME_WEB_FILE;
if (!web || !server || !file) throw new Error('usage: MELDFRAME_WEB_FILE=<a new file name> node properties-e2e.mjs <web-url> <server-url> (see the top of this file; no defaults)');
for (const u of [web, server]) {
  if (!['127.0.0.1', 'localhost', '[::1]'].includes(new URL(u).hostname)) throw new Error(`refusing to send the token to ${u}: the web and server URLs must be loopback (127.0.0.1, localhost or ::1)`);
}
const token = readFileSync(`${root}.dev-token`, 'utf8').trim();
const auth = { authorization: `Bearer ${token}` };
const run = Math.random().toString(36).slice(2, 7);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const pageErrors = [];
const results = [];
let browser;
let n = 0;

async function post(command) {
  const res = await fetch(`${server}/v1/files/${file}/tools/apply_commands`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', ...auth },
    body: JSON.stringify({ ops: [{ key: `props-${run}-${++n}`, command, deps: [] }] }),
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
async function check(name, fn) {
  try {
    await fn();
    results.push(true);
    console.log(`ok   ${name}`);
  } catch (e) {
    results.push(false);
    console.log(`FAIL ${name}: ${String(e.message ?? e).split('\n').slice(0, process.env.MELDFRAME_E2E_DEBUG ? 12 : 1).join(' | ')}`);
  }
}
// Whether React drew the element: React puts a fibre on every DOM node it makes; a Svelte page has none.
const fibre = (page, sel) => page.evaluate((s) => Object.keys(document.querySelector(s) ?? {}).some((k) => k.startsWith('__reactFiber$')), sel);

const frame = `frame_${run}`;
const box = `box_${run}`;
const label = `text_${run}`;
const names = { [frame]: `Card ${run}`, [box]: `Box ${run}`, [label]: `Words ${run}` };

async function open(url, errors = pageErrors) {
  const page = await (await browser.newContext({ viewport: { width: 1280, height: 800 } })).newPage();
  page.on('pageerror', (e) => { errors.push(e.message); console.log('[pageerror]', e.message); });
  await page.goto(url);
  await until('the editor is live', async () => (await page.getAttribute('[data-testid=connection]', 'data-state')) === 'live');
  await until('the nodes are listed', async () => (await page.locator(`[data-testid=layer][data-id="${box}"]`).count()) === 1);
  return page;
}
// The panel shows the node once its name is in the name field (the panel of the node selected before stays up until then).
const select = async (page, id) => {
  await page.locator(`[data-testid=layer][data-id="${id}"] button`).last().click();
  await until('the properties panel shows the node', async () => (await page.locator('[data-testid=prop-name]').inputValue().catch(() => '')) === names[id]);
};
const field = (page, id) => page.locator(`[data-testid=${id}]`);
const type = async (page, id, text) => {
  const f = field(page, id);
  await f.fill(text);
  await f.press('Enter');
};
const undo = (page) => page.locator('[data-testid=undo]').click();

(async () => {
  assert.equal((await snapshot()).seq, 0, 'the file has content already: start the page and the check with a file name that is new');
  await post({ type: 'addNode', node: { id: frame, name: names[frame], type: 'frame', parentId: null, x: 0, y: 0, w: 400, h: 300, fill: '#ffffff' } });
  await post({ type: 'addNode', node: { id: box, name: names[box], type: 'rect', parentId: frame, x: 20, y: 30, w: 80, h: 60, fill: '#335577' } });
  await post({ type: 'addNode', node: { id: label, name: names[label], type: 'text', parentId: frame, x: 20, y: 120, w: 200, h: 40, fill: '#000000', text: 'Hello', fontSize: 20 } });
  browser = await chromium.launch({ args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
  const page = await open(react ? `${web.replace(/\/?$/, '/')}?react=properties` : web);

  await check(react ? 'the Properties panel is drawn by React' : 'the Properties panel is drawn by Svelte', async () => {
    await until('the panel is drawn', async () => (await page.locator('[data-testid=properties]').count()) === 1);
    assert.equal(await fibre(page, '[data-testid=properties]'), react, react ? 'no React fibre: the page fell back to Svelte' : 'a React fibre on the Svelte run');
  });

  await check('with nothing selected the panel says so, and with a node selected it has the sections of the kind', async () => {
    assert.match(await page.locator('[data-testid=properties-empty]').innerText(), /Nothing selected/);
    await select(page, frame);
    const headings = await page.locator('[data-testid=properties] section').evaluateAll((els) => els.map((e) => e.getAttribute('aria-label')));
    assert.deepEqual(headings, ['Position', 'Size', 'Layout', 'Appearance', 'Fill', 'Border', 'Shadow']);
    await select(page, box);
    assert.deepEqual(await page.locator('[data-testid=properties] section').evaluateAll((els) => els.map((e) => e.getAttribute('aria-label'))), ['Position', 'Size', 'Appearance', 'Fill', 'Border', 'Shadow']);
    await select(page, label);
    assert.deepEqual(await page.locator('[data-testid=properties] section').evaluateAll((els) => els.map((e) => e.getAttribute('aria-label'))), ['Position', 'Size', 'Appearance', 'Text', 'Fill', 'Border', 'Shadow']);
  });

  await check('the name, a position and an opacity in percent are sent as typed and show the document\'s value after', async () => {
    await select(page, box);
    await type(page, 'prop-name', `Renamed ${run}`);
    await until('the name on the server', async () => (await nodeOf(box))?.name === `Renamed ${run}`);
    await type(page, 'prop-x', '41.5');
    await until('x on the server', async () => (await nodeOf(box))?.x === 41.5);
    assert.equal(await field(page, 'prop-x').inputValue(), '41.5');
    await type(page, 'prop-opacity', '30');
    await until('opacity on the server', async () => Math.abs((await nodeOf(box))?.opacity - 0.3) < 1e-9);
    assert.equal(await field(page, 'prop-opacity').inputValue(), '30');
    names[box] = `Renamed ${run}`;
  });

  await check('a value the document does not take sends nothing and the field shows the document\'s value again; Escape puts the value back', async () => {
    const before = (await snapshot()).seq;
    await type(page, 'prop-x', 'not a number');
    await until('the field shows the document\'s x again', async () => (await field(page, 'prop-x').inputValue()) === '41.5');
    const f = field(page, 'prop-y');
    const was = await f.inputValue();
    await f.fill('999');
    await f.press('Escape');
    assert.equal(await f.inputValue(), was);
    await sleep(500);
    assert.equal((await snapshot()).seq, before, 'nothing was sent');
  });

  await check('a text left unfinished is sent to the node it was typed for, not to the node selected next', async () => {
    await field(page, 'prop-name').fill(`Left ${run}`);
    // Leaving the field by a click on another layer is a commit (the field reports its text when the person leaves it), made for the node the field showed.
    await page.locator(`[data-testid=layer][data-id="${frame}"] button`).last().click();
    await until('the first node has the name', async () => (await nodeOf(box))?.name === `Left ${run}`);
    names[box] = `Left ${run}`;
    await until('the panel shows the frame', async () => (await field(page, 'prop-name').inputValue()) === names[frame]);
    assert.equal((await nodeOf(frame)).name, names[frame], 'the frame was not renamed');
  });

  await check('the layout direction, Clip content and Add and Remove shadow change the node on the server, and one undo takes each back', async () => {
    await select(page, frame);
    await page.locator('[data-testid=prop-layout]').getByRole('radio', { name: 'Row', exact: true }).click();
    await until('layout row on the server', async () => (await nodeOf(frame))?.layout === 'row');
    await until('the gap field is there', async () => (await field(page, 'prop-gap').count()) === 1);
    await undo(page);
    await until('layout is free again', async () => (await nodeOf(frame))?.layout !== 'row');
    await until('the gap field is gone', async () => (await field(page, 'prop-gap').count()) === 0);
    await field(page, 'prop-clip').click();
    await until('clip false on the server', async () => (await nodeOf(frame))?.clip === false);
    await undo(page);
    await until('clip true again', async () => (await nodeOf(frame))?.clip === true);
    await field(page, 'prop-shadow-add').click();
    await until('one shadow on the server', async () => (await nodeOf(frame))?.shadows?.length === 1);
    await until('a shadow row is drawn', async () => (await field(page, 'prop-shadow').count()) === 1);
    await field(page, 'prop-shadow-remove').click();
    await until('no shadow on the server', async () => !(await nodeOf(frame))?.shadows?.length);
    await until('the shadow row is gone', async () => (await field(page, 'prop-shadow').count()) === 0);
  });

  await check('the fields follow a change of an agent', async () => {
    await select(page, box);
    await post({ type: 'updateNode', id: box, patch: { x: 77, w: 123 } });
    await until('the x field follows', async () => (await field(page, 'prop-x').inputValue()) === '77');
    await until('the width field follows', async () => (await field(page, 'prop-w').inputValue()) === '123');
  });
  await page.context().close();

  if (react) {
    await check('with the chunks of the React copy blocked, the Svelte panel is drawn and works', async () => {
      const errors = [];
      const ctx = await browser.newContext({ viewport: { width: 1280, height: 800 } });
      const blocked = await ctx.newPage();
      blocked.on('pageerror', (e) => { errors.push(e.message); console.log('[pageerror]', e.message); });
      await blocked.route((u) => /react\/(PropertiesPanel|island)/.test(u.pathname), (route) => route.abort('failed'));
      await blocked.goto(`${web.replace(/\/?$/, '/')}?react=properties`);
      await until('the editor is live', async () => (await blocked.getAttribute('[data-testid=connection]', 'data-state')) === 'live');
      await until('the nodes are listed', async () => (await blocked.locator(`[data-testid=layer][data-id="${box}"]`).count()) === 1);
      await until('the panel is drawn', async () => (await blocked.locator('[data-testid=properties]').count()) === 1);
      assert.equal(await fibre(blocked, '[data-testid=properties]'), false, 'the panel is the Svelte copy');
      await select(blocked, box);
      await type(blocked, 'prop-w', '150');
      await until('the width on the server', async () => (await nodeOf(box))?.w === 150);
      await ctx.close();
      // The errors that the aborted requests raise in the page (a failed dynamic import is caught by the switch) are the one thing allowed here.
      assert.deepEqual(errors.filter((m) => !/Failed to fetch dynamically imported module|Importing a module script failed|error loading dynamically imported module/i.test(m)), []);
    });
  }

  await check('no uncaught error in the page', async () => assert.deepEqual(pageErrors, []));
  await browser.close();
  const failed = results.filter((r) => !r).length;
  console.log(failed ? `${failed} of ${results.length} checks failed` : `all ${results.length} checks passed`);
  process.exit(failed ? 1 : 0);
})().catch(async (e) => {
  console.error(`properties-e2e: ${e.stack ?? e.message}`);
  await browser?.close().catch(() => {});
  process.exit(1);
});
