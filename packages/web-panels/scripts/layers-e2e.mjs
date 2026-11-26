// Browser check of the Layers panel against a real server: node packages/web-panels/scripts/layers-e2e.mjs [web-url] [server-url]
// Needs the development server (`just dev`, or the server binary with MELDFRAME_DEV_TOKENS and no DATABASE_URL, token in
// .dev-token at the root) and `just web` (Vite) running, the WASM bridge built, and Playwright from the global module path.
// It adds one frame with two children to the file `MELDFRAME_WEB_FILE` (default `file_dev`) under names of its own, and selects,
// folds and unfolds them: run it against the development server only. Hand-run, not in CI.
// The React copy of the panel (E7.3 slice S5) is checked by the same script with MELDFRAME_UI=react: it opens the page with
// /?react=layers, and a first check asks that React really drew the panel (a silent fallback to Svelte would pass every other
// check). Without it the Svelte copy runs. With React, a last check blocks the chunks of the React copy and asks that the Svelte
// panel is drawn and works, so that a chunk that does not load never leaves a page with no layers. The block matches the paths of the Vite dev
// server's modules (`.../react/LayersPanel.tsx`), so that check is for the dev server only and not for a production build (whose chunks have hashed names).
// A second token among the server's development tokens, with another actor (MELDFRAME_E2E_TOKEN2, as editor-e2e.mjs has it), adds a check of the
// marker of who changed a node; without it that check prints SKIP and counts as neither passed nor failed.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';

const { chromium } = createRequire(import.meta.url)('playwright');
const react = process.env.MELDFRAME_UI === 'react';
const web = process.argv[2] ?? 'http://localhost:5173';
const server = process.argv[3] ?? 'http://127.0.0.1:8080';
const root = new URL('../../../', import.meta.url).pathname;
const token = readFileSync(`${root}.dev-token`, 'utf8').trim();
const file = process.env.MELDFRAME_WEB_FILE ?? 'file_dev';
const run = Math.random().toString(36).slice(2, 8);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const pageErrors = [];
const results = [];
let n = 0;

const token2 = process.env.MELDFRAME_E2E_TOKEN2;
async function post(command, as = token) {
  const res = await fetch(`${server}/v1/files/${file}/tools/apply_commands`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', authorization: `Bearer ${as}` },
    body: JSON.stringify({ ops: [{ key: `layers-e2e-${run}-${++n}`, command, deps: [] }] }),
  });
  assert.equal(res.status, 200, `op refused: ${res.status} ${await res.text()}`);
}
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
  try { await fn(); results.push(true); console.log(`ok   ${name}`); } catch (e) { results.push(false); console.log(`FAIL ${name}: ${String(e.message ?? e).split('\n')[0]}`); }
}
// Whether React drew the element: React puts a fibre on every DOM node it makes; a Svelte page has none.
const fibre = (page, sel) => page.evaluate((s) => Object.keys(document.querySelector(s) ?? {}).some((k) => k.startsWith('__reactFiber$')), sel);

const frame = `lf_${run}`;
const one = `l1_${run}`;
const two = `l2_${run}`;
const names = { [frame]: `Frame ${run}`, [one]: `Alpha ${run}`, [two]: `Beta ${run}` };
const rowOf = (page, id) => page.locator(`[data-testid=layer][data-id="${id}"]`);
const selected = (page, id) => rowOf(page, id).getAttribute('data-selected');

async function open(browser, url, viewport = { width: 1280, height: 800 }) {
  const page = await (await browser.newContext({ viewport })).newPage();
  page.on('pageerror', (e) => { pageErrors.push(e.message); console.log('[pageerror]', e.message); });
  await page.goto(url);
  await until('the editor is live', async () => (await page.getAttribute('[data-testid=connection]', 'data-state')) === 'live');
  await until('the frame is listed', async () => (await rowOf(page, frame).count()) === 1);
  return page;
}

(async () => {
  await post({ type: 'addNode', node: { id: frame, name: names[frame], type: 'frame', parentId: null, x: 0, y: 0, w: 400, h: 300, fill: '#ffffff' } });
  await post({ type: 'addNode', node: { id: one, name: names[one], type: 'rect', parentId: frame, x: 10, y: 10, w: 80, h: 60, fill: '#335577' } });
  await post({ type: 'addNode', node: { id: two, name: names[two], type: 'ellipse', parentId: frame, x: 100, y: 10, w: 80, h: 60, fill: '#775533' } });
  const browser = await chromium.launch({ args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
  const page = await open(browser, react ? `${web.replace(/\/?$/, '/')}?react=layers` : web);

  await check('the panel is drawn by the copy that was asked for', async () => {
    assert.equal(await page.locator('[data-testid=layers]').count(), 1);
    assert.equal(await fibre(page, '[data-testid=layers]'), react, react ? 'the Layers panel has no React fibre: the page fell back to Svelte' : 'the Layers panel has a React fibre on the Svelte run');
  });
  await check('the rows are named, indented under their parent, and the children come front first', async () => {
    const all = await page.locator('[data-testid=layer]').evaluateAll((els) => els.map((e) => e.getAttribute('data-id')));
    const mine = all.filter((id) => [frame, one, two].includes(id));
    assert.deepEqual(mine, [frame, two, one], 'the frame first, then its children, the front one first');
    assert.equal(await page.getByRole('list', { name: 'Layers' }).count(), 1, 'the list is named by its title');
    const pad = (id) => rowOf(page, id).evaluate((e) => Number.parseFloat(getComputedStyle(e).paddingLeft));
    assert.ok((await pad(one)) > (await pad(frame)), 'a child is indented');
    assert.ok((await rowOf(page, one).textContent()).includes(names[one]));
    assert.equal(await rowOf(page, frame).locator('button[aria-expanded]').getAttribute('aria-label'), `Collapse ${names[frame]}`);
    assert.equal(await rowOf(page, one).locator('button[aria-expanded]').count(), 0, 'a leaf has no chevron');
  });
  await check('a click selects a row and the properties show it; Shift adds a row to the selection', async () => {
    await rowOf(page, one).locator('button').last().click();
    await until('the row is selected', async () => (await selected(page, one)) === 'true');
    assert.equal(await selected(page, two), 'false');
    assert.equal(await rowOf(page, one).locator('button').last().getAttribute('aria-current'), 'true');
    await until('the properties show the node', async () => (await page.inputValue('[data-testid=prop-name]').catch(() => '')) === names[one]);
    await rowOf(page, two).locator('button').last().click({ modifiers: ['Shift'] });
    await until('both rows are selected', async () => (await selected(page, one)) === 'true' && (await selected(page, two)) === 'true');
    await rowOf(page, two).locator('button').last().click();
    await until('one row is selected', async () => (await selected(page, one)) === 'false' && (await selected(page, two)) === 'true');
  });
  await check('the keyboard reaches a row and Enter selects it', async () => {
    await rowOf(page, one).locator('button').last().focus();
    await page.keyboard.press('Enter');
    await until('the row is selected', async () => (await selected(page, one)) === 'true' && (await selected(page, two)) === 'false');
  });
  await check('the chevron folds a parent and hides its children, and unfolds them again', async () => {
    const chevron = rowOf(page, frame).locator('button[aria-expanded]');
    assert.equal(await chevron.getAttribute('aria-expanded'), 'true');
    await chevron.click();
    await until('the children go', async () => (await rowOf(page, one).count()) === 0 && (await rowOf(page, two).count()) === 0);
    assert.equal(await chevron.getAttribute('aria-label'), `Expand ${names[frame]}`);
    assert.equal(await chevron.getAttribute('aria-expanded'), 'false');
    await chevron.click();
    await until('the children are back', async () => (await rowOf(page, one).count()) === 1 && (await rowOf(page, two).count()) === 1);
    assert.equal(await chevron.getAttribute('aria-label'), `Collapse ${names[frame]}`);
  });
  await check('a change from outside shows in the rows: a rename, and a node that arrives', async () => {
    await post({ type: 'updateNode', id: one, patch: { name: `${names[one]} renamed` } });
    await until('the new name', async () => (await rowOf(page, one).textContent()).includes(`${names[one]} renamed`));
    const late = `l3_${run}`;
    await post({ type: 'addNode', node: { id: late, name: `Late ${run}`, type: 'text', parentId: frame, x: 10, y: 100, w: 120, h: 30, text: 'Hello', fontSize: 16, fill: '#ffffff' } });
    await until('the new row', async () => (await rowOf(page, late).count()) === 1);
  });
  if (token2) {
    await check('a node changed by another actor shows who, and a node of this page does not', async () => {
      const other = `l4_${run}`;
      await post({ type: 'addNode', node: { id: other, name: `Other ${run}`, type: 'rect', parentId: frame, x: 10, y: 140, w: 20, h: 20, fill: '#335577' } }, token2);
      await until('the new row', async () => (await rowOf(page, other).count()) === 1);
      const marker = rowOf(page, other).locator('[data-testid=actor]');
      assert.equal(await marker.count(), 1, 'the row names the actor');
      assert.match(await marker.getAttribute('title'), /^Last changed by \S/);
      assert.equal(await rowOf(page, one).locator('[data-testid=actor]').count(), 0, 'a node of this page has no marker');
    });
  } else {
    console.log('SKIP the marker of another actor: MELDFRAME_E2E_TOKEN2 is not set (a second token with another actor among the server tokens)');
  }
  await page.context().close();

  if (react) {
    await check('with the chunks of the React copy blocked, the Svelte panel is drawn and works', async () => {
      const ctx = await browser.newContext({ viewport: { width: 1280, height: 800 } });
      const blocked = await ctx.newPage();
      blocked.on('pageerror', (e) => { pageErrors.push(e.message); console.log('[pageerror]', e.message); });
      await blocked.route((u) => /react\/(LayersPanel|island)/.test(u.pathname), (route) => route.abort('failed'));
      await blocked.goto(`${web.replace(/\/?$/, '/')}?react=layers`);
      await until('the editor is live', async () => (await blocked.getAttribute('[data-testid=connection]', 'data-state')) === 'live');
      await until('the frame is listed', async () => (await rowOf(blocked, frame).count()) === 1);
      assert.equal(await fibre(blocked, '[data-testid=layers]'), false, 'the panel is the Svelte copy');
      await rowOf(blocked, two).locator('button').last().click();
      await until('the row is selected', async () => (await selected(blocked, two)) === 'true');
      await ctx.close();
    });
  }

  await check('no uncaught error in the page', async () => assert.deepEqual(pageErrors, []));
  await browser.close();
  const failed = results.filter((r) => !r).length;
  console.log(failed ? `${failed} of ${results.length} checks failed` : `all ${results.length} checks passed`);
  process.exit(failed ? 1 : 0);
})().catch((e) => {
  console.error(`layers-e2e: ${e.message}`);
  process.exit(1);
});
