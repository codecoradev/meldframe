// Browser check of the text being edited (T12, decision 43 clause Q6) against a real server and the real WASM bridge:
//   node packages/web-e2e/scripts/text-edit-e2e.mjs <web-url> <server-url>
// Needs the development server (`just dev`, token in .dev-token), the web client (`just web`) opened on the file named in
// MELDFRAME_WEB_FILE, which must be new for each run (it adds a frame, two texts and a rectangle to it), the WASM bridge built
// from this revision (`just wasm-build`) and Playwright from the global module path (NODE_PATH). It asks the page what a person
// sees (the field, the notice, the undo button) and the server what was written. The begin, commit and cancel are the app
// layer's (`state.textEdit`); the field keeps the keystrokes. It guards the notice of a node that
// someone else deleted while its field was open (the app raises `Refused` `not_found`, so the notice does not depend on the
// blur of a field that is removed); it shows also when nothing was typed (accepted).
// MELDFRAME_UI=react opens the page with the React copies of the panels (`react=feedback,layers,variables,properties,canvas`).
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';

const { chromium } = createRequire(import.meta.url)('playwright');
const web = process.argv[2];
const server = process.argv[3];
assert.ok(web && server, 'usage: text-edit-e2e.mjs <web-url> <server-url>');
const root = new URL('../../../', import.meta.url).pathname;
const token = readFileSync(`${root}.dev-token`, 'utf8').trim();
const file = process.env.MELDFRAME_WEB_FILE ?? 'file_dev';
const react = process.env.MELDFRAME_UI === 'react';
const run = Math.random().toString(36).slice(2, 8);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const pageErrors = [];
let n = 0;
const results = [];

async function post(command) {
  const res = await fetch(`${server}/v1/files/${file}/tools/apply_commands`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', authorization: `Bearer ${token}` },
    body: JSON.stringify({ ops: [{ key: `te-${run}-${++n}`, command, deps: [] }] }),
  });
  assert.equal(res.status, 200, `op refused: ${res.status} ${await res.text()}`);
}
const nodeOf = async (id) => {
  const res = await fetch(`${server}/v1/files/${file}`, { headers: { authorization: `Bearer ${token}` } });
  return (await res.json()).nodes.find((x) => x.id === id);
};
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
  try { await fn(); results.push(true); console.log('ok  ', name); } catch (e) { results.push(false); console.log('FAIL', name, '\n    ', String(e.message ?? e).split('\n')[0]); }
}
/** The box on the screen of the frame (the pixels of its fill #12ab34), which lies at the world origin. */
async function bbox(page) {
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
  }, { b64, want: [0x12, 0xab, 0x34] });
}

(async () => {
  const [F, T1, T2, T3, R] = ['frame', 't1', 't2', 't3', 'rect'].map((s) => `te_${run}_${s}`);
  const text = (id, name, y, t) => ({ type: 'addNode', node: { id, name, type: 'text', parentId: F, x: 20, y, w: 200, h: 40, text: t, fontSize: 24, fill: '#ffffff' } });
  await post({ type: 'addNode', node: { id: F, name: 'Frame', type: 'frame', parentId: null, x: 0, y: 0, w: 400, h: 300, fill: '#12ab34' } });
  await post(text(T1, 'One', 20, 'Hello'));
  await post(text(T2, 'Two', 90, 'World'));
  await post(text(T3, 'Three', 160, 'Gone'));
  await post({ type: 'addNode', node: { id: R, name: 'Box', type: 'rect', parentId: F, x: 250, y: 20, w: 100, h: 60, fill: '#ab1234' } });
  const browser = await chromium.launch({ args: ['--use-gl=angle', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1 });
  page.on('pageerror', (e) => { pageErrors.push(e.message); console.log('[pageerror]', e.message); });
  await page.goto(react ? `${web}/?react=feedback,layers,variables,properties,canvas` : web);
  const box = await until('the frame is drawn', () => bbox(page));
  await until('the three texts are on the server', async () => (await nodeOf(T3)) && (await nodeOf(R)));
  await sleep(600);
  const z = Number.parseInt(await page.textContent('[data-testid=zoom]'), 10) / 100;
  const at = (x, y) => ({ x: box.x0 + x * z, y: box.y0 + y * z });
  const field = page.locator('[data-testid=text-editor]');
  const undo = page.locator('[data-testid=undo]');
  const open = async (p) => { await page.mouse.dblclick(p.x, p.y); await field.waitFor({ state: 'visible', timeout: 5000 }); };
  const [p1, p2, p3, pr, pfree] = [at(40, 40), at(40, 110), at(40, 180), at(300, 50), at(300, 250)];
  const closed = async () => until('the field is closed', async () => (await field.count()) === 0);

  await check('a click elsewhere (the field loses focus) commits one change; one undo takes it back', async () => {
    await open(p1);
    assert.equal(await field.inputValue(), 'Hello');
    assert.equal(await undo.isDisabled(), true);
    await field.fill('Hello there');
    await page.mouse.click(pfree.x, pfree.y);
    await until('the new text on the server', async () => (await nodeOf(T1)).text === 'Hello there');
    await closed();
    assert.equal(await undo.isDisabled(), false, 'one step to undo');
    await page.keyboard.press('Control+z');
    await until('one undo', async () => (await nodeOf(T1)).text === 'Hello');
    assert.equal(await undo.isDisabled(), true, 'that was the only step');
  });
  await check('Escape cancels: nothing written, no undo step, the field is closed', async () => {
    await open(p1);
    await field.fill('Never saved');
    await page.keyboard.press('Escape');
    await closed();
    await sleep(400);
    assert.equal((await nodeOf(T1)).text, 'Hello');
    assert.equal(await undo.isDisabled(), true);
  });
  await check('Ctrl+Enter on an unchanged text writes nothing and makes no undo step', async () => {
    await open(p1);
    await page.keyboard.press('Control+Enter');
    await closed();
    assert.equal((await nodeOf(T1)).text, 'Hello');
    assert.equal(await undo.isDisabled(), true);
  });
  await check('a double click on a rectangle opens no field and says nothing', async () => {
    await page.mouse.dblclick(pr.x, pr.y);
    await sleep(500);
    assert.equal(await field.count(), 0);
    assert.equal(await page.locator('[data-testid=notice]').count(), 0, 'a notice was shown');
  });
  await check('Enter edits a selected text and not a selected rectangle', async () => {
    await page.mouse.click(pr.x, pr.y);
    await page.keyboard.press('Enter');
    await sleep(400);
    assert.equal(await field.count(), 0, 'Enter on a rectangle opened a field');
    await page.mouse.click(p1.x, p1.y);
    await page.keyboard.press('Enter');
    await field.waitFor({ state: 'visible', timeout: 5000 });
    assert.equal(await field.inputValue(), 'Hello');
    await page.keyboard.press('Escape');
    await closed();
  });
  await check('a change of the node by someone else leaves the field as it is; an equal text writes nothing, a different one is written over theirs', async () => {
    await open(p1);
    await post({ type: 'updateNode', id: T1, patch: { text: 'Agent' } });
    await until('their text on the server', async () => (await nodeOf(T1)).text === 'Agent');
    await sleep(500);
    assert.equal(await field.count(), 1, 'the field stayed open');
    assert.equal(await field.inputValue(), 'Hello', 'and holds what it held');
    await page.keyboard.press('Control+Enter');
    await closed();
    await sleep(400);
    assert.equal((await nodeOf(T1)).text, 'Agent', 'an unchanged text did not overwrite their change');
    await open(p1);
    assert.equal(await field.inputValue(), 'Agent');
    await field.fill('Mine');
    await page.keyboard.press('Control+Enter');
    await until('my text on the server', async () => (await nodeOf(T1)).text === 'Mine');
    await closed();
  });
  await check('a double click on another text while one is edited commits the first and opens the second', async () => {
    await open(p1);
    await field.fill('First done');
    await page.mouse.dblclick(p2.x, p2.y);
    await until('the first text on the server', async () => (await nodeOf(T1)).text === 'First done');
    await until('the second field shows World', async () => (await field.count()) === 1 && (await field.inputValue()) === 'World');
    await page.keyboard.press('Escape');
    await closed();
  });
  await check('when the node is deleted by someone else the field closes, nothing is written and the notice says the node is gone', async () => {
    assert.equal(await page.locator('[data-testid=notice]').count(), 0, 'a notice is up before the deletion');
    await open(p3);
    await field.fill('typed for a node that is going');
    await post({ type: 'removeNode', id: T3 });
    await until('the node is gone', async () => !(await nodeOf(T3)));
    await closed();
    const notice = page.locator('[data-testid=notice]');
    await notice.waitFor({ state: 'visible', timeout: 5000 });
    assert.match((await notice.textContent()) ?? '', /That node no longer exists\./);
    await sleep(500);
    assert.equal(await notice.count(), 1, 'one notice');
    assert.equal(await nodeOf(T3), undefined, 'the text typed for the deleted node was not written');
  });
  await check('an input method composition stays in the field: nothing is sent while composing, and the composed text is what a commit writes', async () => {
    await open(p1);
    await field.press('End');
    const cdp = await page.context().newCDPSession(page);
    await cdp.send('Input.imeSetComposition', { text: 'ni', selectionStart: 2, selectionEnd: 2 });
    await sleep(300);
    assert.equal((await nodeOf(T1)).text, 'First done', 'nothing was written while composing');
    assert.equal(await field.count(), 1, 'the field is still open');
    await cdp.send('Input.insertText', { text: '\u65e5\u672c' });
    await sleep(200);
    assert.equal(await field.inputValue(), 'First done\u65e5\u672c', 'the composed text is in the field');
    await page.keyboard.press('Control+Enter');
    await until('the composed text on the server', async () => (await nodeOf(T1)).text === 'First done\u65e5\u672c');
    await closed();
  });
  await check('no uncaught error in the page', async () => assert.deepEqual(pageErrors, []));
  await browser.close();
  const failed = results.filter((r) => !r).length;
  console.log(failed ? `${failed} of ${results.length} checks failed` : `all ${results.length} checks passed`);
  process.exit(failed ? 1 : 0);
})().catch((e) => { console.error(`text-edit-e2e: ${e.message}`); process.exit(1); });
