// Browser check of the weight and the alignment of a text and of the clip switch of a frame, against a real server:
//   MELDFRAME_WEB_FILE=<a new file name> node packages/web-panels/scripts/text-style-e2e.mjs <web-url> <server-url>
// Issue 252: the Text section has a Font weight select (the nine weights the model takes) and a Text alignment group (left, center,
// right); the Appearance section of a frame has Clip content. The check selects a text and sets weight 700 and alignment center with
// the pointer and with the keyboard, reads the node back from the server (fontWeight, textAlign), checks that the canvas draws it (a
// bolder text has more ink; a centred and a right-aligned text start further right in the same box), that one undo each brings the
// old value back, that a value the document does not take (fed to the select through the page) sends nothing and the select shows the
// document's value again, that the server refuses a weight of 450 with 422 and E_INVALID_FONT_WEIGHT, and the hit sizes and labels of the controls. Then it switches Clip content
// off on a frame with a child that overflows and checks the server and that the canvas draws the part outside; on again hides it.
// Start the server and the page as the header of packages/web-panels/scripts/image-fill-e2e.mjs says: a file name that is new for
// every run (the check refuses a file that has content), the server on a free port, the development token of the page in
// .dev-token at the root and in the server's token list, the page's URL and the server's URL as the two arguments, no defaults.
// The WASM bridge must be built from this revision (`just wasm-build`), and Playwright comes from the global module path. Both URLs must
// be loopback (the token is sent there). It adds a frame and three nodes to the file and leaves them: development server only.
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
if (!web || !server || !file) throw new Error('usage: MELDFRAME_WEB_FILE=<a new file name> node text-style-e2e.mjs <web-url> <server-url> (see the top of this file; no defaults)');
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
    body: JSON.stringify({ ops: [{ key: `ts-${run}-${++n}`, command, deps: [] }] }),
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
const MAGENTA = [255, 0, 255]; // the text
const RED = [255, 0, 0]; // the child that overflows

const $ = (sel) => page.locator(sel);
const results = [];
async function check(name, fn) {
  try {
    await fn();
    results.push(true);
    console.log(`ok   ${name}`);
  } catch (e) {
    results.push(false);
    if (shots && page) await page.screenshot({ path: `${shots}/text-style-fail-${results.length}.png` }).catch(() => {});
    console.log(`FAIL ${name}: ${String(e.message ?? e).split('\n').slice(0, process.env.MELDFRAME_E2E_DEBUG ? 12 : 1).join(' | ')}`);
  }
}
const shot = (name) => (shots && page ? page.screenshot({ path: `${shots}/text-style-${name}.png` }) : null);

const frame = `frame_${run}`;
const label = `text_${run}`;
const over = `over_${run}`;
// The panel shows the node once its name is in the name field (the panel of the node selected before stays up until then).
const select = async (id, name) => {
  await $(`[data-testid=layer][data-id="${id}"] button`).last().click();
  await until('the properties panel shows the node', async () => (await $('[data-testid=prop-name]').inputValue().catch(() => '')) === name);
};
const align = (name) => $('[data-testid=prop-textAlign]').getByRole('radio', { name, exact: true });
const undo = async () => {
  await page.locator('[data-testid=undo]').click();
};

(async () => {
  assert.equal((await snapshot()).seq, 0, 'the file has content already: start the page and the check with a file name that is new');
  await post({ type: 'addNode', node: { id: frame, name: `Card ${run}`, type: 'frame', parentId: null, x: 0, y: 0, w: 300, h: 200, fill: '#ffffff' } });
  await post({ type: 'addNode', node: { id: label, name: `Words ${run}`, type: 'text', parentId: frame, x: 20, y: 20, w: 260, h: 40, fill: '#ff00ff', text: 'Hello', fontSize: 24 } });
  await post({ type: 'addNode', node: { id: over, name: `Over ${run}`, type: 'rect', parentId: frame, x: 200, y: 100, w: 200, h: 200, fill: '#ff0000' } });
  browser = await chromium.launch({ args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
  page = await browser.newPage({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1 });
  page.on('pageerror', (e) => { pageErrors.push(e.message); console.log('[pageerror]', e.message); });
  await page.goto(react ? `${web.replace(/\/?$/, '/')}?react=properties` : web);
  await until('the editor is live', async () => (await page.getAttribute('[data-testid=connection]', 'data-state')) === 'live');
  await until('the nodes are listed', async () => (await $(`[data-testid=layer][data-id="${over}"]`).count()) === 1);
  // MELDFRAME_UI=react: the page must really be the React copy of the Properties panel (a fallback to Svelte would pass every other check).
  if (react) await page.waitForSelector('[data-testid=properties]', { timeout: 20_000 }); // the React chunk loads on demand: the panel is there a moment after the editor is live
  if (react) assert.equal(await page.evaluate(() => Object.keys(document.querySelector('[data-testid=properties]') ?? {}).some((k) => k.startsWith('__reactFiber$'))), true, 'the Properties panel has no React fibre: the page fell back to Svelte');
  let regular;

  await check('a text has the Font weight select with the nine weights and the Text alignment group; no other kind has them', async () => {
    await select(over, `Over ${run}`);
    assert.equal(await $('[data-testid=prop-fontWeight]').count(), 0, 'a rectangle has no weight');
    assert.equal(await $('[data-testid=prop-textAlign]').count(), 0, 'a rectangle has no alignment');
    await select(frame, `Card ${run}`);
    assert.equal(await $('[data-testid=prop-fontWeight]').count(), 0, 'a frame has no weight');
    await select(label, `Words ${run}`);
    const sel = $('[data-testid=prop-fontWeight]');
    assert.equal(await page.getByLabel('Font weight', { exact: true }).count(), 1, 'the select is named by its caption');
    assert.equal(await sel.getAttribute('aria-label'), null, 'one name: the caption, no aria-label over it');
    assert.equal(await page.getByRole('radiogroup', { name: 'Text alignment', exact: true }).count(), 1);
    assert.deepEqual(await sel.locator('option').evaluateAll((o) => o.map((x) => x.value)), ['100', '200', '300', '400', '500', '600', '700', '800', '900']);
    assert.equal(await sel.inputValue(), '400', 'the select shows the weight of the node');
    assert.deepEqual(await $('[data-testid=prop-textAlign] [role=radio]').evaluateAll((r) => r.map((x) => x.getAttribute('aria-label'))), ['Align left', 'Align center', 'Align right']);
    assert.equal(await align('Align left').getAttribute('aria-checked'), 'true');
    for (const box of [await sel.boundingBox(), ...(await Promise.all([align('Align left'), align('Align center'), align('Align right')].map((r) => r.boundingBox())))]) {
      assert.ok(box && box.height >= 24 && box.width >= 24, `a hit target of at least 24 px: ${JSON.stringify(box)}`);
    }
    regular = await ink(MAGENTA);
    assert.ok(regular.count > 50, `the canvas draws the text (${regular.count} pixels)`);
    await shot('text-selected');
  });

  await check('weight 700 chosen in the select is on the server as fontWeight 700 and the canvas draws more ink', async () => {
    await $('[data-testid=prop-fontWeight]').selectOption('700');
    await until('fontWeight 700 on the server', async () => (await nodeOf(label))?.fontWeight === 700);
    await until('the canvas shows a bolder text', async () => (await ink(MAGENTA)).count > regular.count * 1.05);
    assert.equal(await $('[data-testid=prop-fontWeight]').inputValue(), '700');
    await shot('bold');
  });

  await check('alignment center is on the server as textAlign center and the text starts further right in its box; right goes further still', async () => {
    const before = await ink(MAGENTA);
    await align('Align center').click();
    await until('textAlign center on the server', async () => (await nodeOf(label))?.textAlign === 'center');
    const centre = await until('the text moved right', async () => { const i = await ink(MAGENTA); return i.min > before.min + 10 ? i : null; });
    assert.equal(await align('Align center').getAttribute('aria-checked'), 'true');
    await align('Align right').click();
    await until('textAlign right on the server', async () => (await nodeOf(label))?.textAlign === 'right');
    const right = await until('the text moved further right', async () => { const i = await ink(MAGENTA); return i.min > centre.min + 10 ? i : null; });
    // The same box and the same text: left to centre and centre to right are the same distance, and the right edge of the box is passed by neither.
    assert.ok(Math.abs((centre.min - before.min) - (right.min - centre.min)) <= 4, `centre ${centre.min - before.min} px, then ${right.min - centre.min} px`);
    assert.ok(right.max >= centre.max, 'the right end of the ink moves right too');
    await shot('right');
  });

  await check('the keyboard works the controls: ArrowLeft on the checked radio chooses center, and ArrowDown on the select chooses the next weight', async () => {
    await align('Align right').focus();
    await page.keyboard.press('ArrowLeft');
    await until('textAlign center on the server', async () => (await nodeOf(label))?.textAlign === 'center');
    assert.equal(await align('Align center').evaluate((e) => document.activeElement === e), true, 'the focus follows the choice');
    await $('[data-testid=prop-fontWeight]').focus();
    await page.keyboard.press('ArrowDown');
    await until('fontWeight 800 on the server', async () => (await nodeOf(label))?.fontWeight === 800);
  });

  await check('one undo each brings back the old weight and the old alignment, and the controls follow', async () => {
    // The steps so far, newest first: weight 800, align center (by key), align right, align center, weight 700.
    await undo();
    await until('weight back to 700', async () => (await nodeOf(label))?.fontWeight === 700);
    assert.equal(await $('[data-testid=prop-fontWeight]').inputValue(), '700');
    await undo();
    await until('align back to right', async () => (await nodeOf(label))?.textAlign === 'right');
    assert.equal(await align('Align right').getAttribute('aria-checked'), 'true');
    await undo();
    await undo();
    await until('align back to left', async () => (await nodeOf(label))?.textAlign === 'left');
    await undo();
    await until('weight back to 400', async () => (await nodeOf(label))?.fontWeight === 400);
    assert.equal(await $('[data-testid=prop-fontWeight]').inputValue(), '400');
    assert.equal(await align('Align left').getAttribute('aria-checked'), 'true');
    await shot('after-undo');
    await until('the canvas is as it was', async () => { const i = await ink(MAGENTA); if (process.env.MELDFRAME_E2E_DEBUG) console.log(JSON.stringify({ now: i, regular })); return Math.abs(i.min - regular.min) <= 1 && Math.abs(i.count - regular.count) <= regular.count * 0.02; }, 5000);
  });

  await check('a text of another weight written by an agent (apply_commands) shows in the select', async () => {
    await post({ type: 'updateNode', id: label, patch: { fontWeight: 300 } });
    await until('the select shows 300', async () => (await $('[data-testid=prop-fontWeight]').inputValue()) === '300');
  });

  await check('a value the document does not take, fed to the select through the page, sends nothing and the select shows the document value again', async () => {
    const seq = (await snapshot()).seq;
    // The select offers nine weights; the page adds a tenth (450) and chooses it, so that the change handler runs with a weight the model refuses.
    await $('[data-testid=prop-fontWeight]').evaluate((el) => {
      const o = document.createElement('option');
      o.value = '450';
      o.textContent = 'Not a weight';
      el.append(o);
      el.value = '450';
      el.dispatchEvent(new Event('change', { bubbles: true }));
    });
    await until('the select shows 300 again', async () => (await $('[data-testid=prop-fontWeight]').inputValue()) === '300');
    await sleep(500);
    assert.equal((await snapshot()).seq, seq, 'nothing was sent');
    assert.equal((await nodeOf(label)).fontWeight, 300);
  });

  await check('the server refuses a weight of 450 with 422 and E_INVALID_FONT_WEIGHT, and the node keeps its weight', async () => {
    const res = await fetch(`${server}/v1/files/${file}/tools/apply_commands`, { method: 'POST', headers: { 'content-type': 'application/json', ...auth }, body: JSON.stringify({ ops: [{ key: `ts-${run}-bad`, command: { type: 'updateNode', id: label, patch: { fontWeight: 450 } }, deps: [] }] }) });
    assert.equal(res.status, 422);
    assert.equal((await res.json()).error.code, 'E_INVALID_FONT_WEIGHT');
    assert.equal((await nodeOf(label)).fontWeight, 300);
  });

  await check('Clip content: a frame has it and no other kind has; switching it off is clip false on the server and the canvas draws the part of the child outside the frame', async () => {
    await select(label, `Words ${run}`);
    assert.equal(await $('[data-testid=prop-clip]').count(), 0, 'a text does not clip');
    await select(over, `Over ${run}`);
    assert.equal(await $('[data-testid=prop-clip]').count(), 0, 'a rectangle does not clip');
    await select(frame, `Card ${run}`);
    const clip = $('[data-testid=prop-clip]');
    assert.equal(await clip.isChecked(), true, 'the model has clip on by default');
    const clipped = await ink(RED);
    assert.ok(clipped.count > 50, `the part of the child inside the frame is drawn (${clipped.count} pixels)`);
    await clip.click();
    await until('clip false on the server', async () => (await nodeOf(frame))?.clip === false);
    await until('the canvas draws the child outside the frame', async () => { const i = await ink(RED); return i.count > clipped.count * 1.5; });
    await shot('clip-off');
    await clip.click();
    await until('clip true on the server', async () => (await nodeOf(frame))?.clip === true);
    await until('the canvas cuts the child again', async () => Math.abs((await ink(RED)).count - clipped.count) <= clipped.count * 0.02);
    await undo();
    await until('one undo is clip false again', async () => (await nodeOf(frame))?.clip === false);
    assert.equal(await clip.isChecked(), false);
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
