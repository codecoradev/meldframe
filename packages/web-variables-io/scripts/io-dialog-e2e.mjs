// Browser check of the shell of the Export variables and Import variables dialogs against a real server: node packages/web-variables-io/scripts/io-dialog-e2e.mjs [web-url] [server-url]
// What each dialog does as a dialog, whoever draws it (E7.3 slice S10): its name and description, where the focus starts, Tab and Shift+Tab, the scrim
// and the scroll lock, the three ways to close it and where the focus goes back to, what Copy said not coming back, and the keys typed in it being text.
// io-e2e.mjs checks what is done inside them (the export text, the import and its report, the file chooser).
// MELDFRAME_UI=react opens the page with /?react=io (MELDFRAME_REACT_PANELS names other switch words, such as variables,manage,io) and a first check asks that
// React really drew each dialog (a silent fallback to Svelte would pass every other check); without it the Svelte dialogs (Bits UI) run.
// Start the server and the page as manage-e2e.mjs says (its header has the commands) with a file name that is new for every run; the token of the page is in
// .dev-token at the root, the web and server URLs must be loopback, the WASM bridge is built from this revision and Playwright comes from the global module
// path. It adds one collection and one variable to the file and leaves them: run it against a development server only.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';

const { chromium } = createRequire(import.meta.url)('playwright');
const react = process.env.MELDFRAME_UI === 'react';
const words = process.env.MELDFRAME_REACT_PANELS ?? 'io';
const web = process.argv[2] ?? 'http://localhost:5173';
const server = process.argv[3] ?? 'http://127.0.0.1:8080';
const root = new URL('../../../', import.meta.url).pathname;
const token = readFileSync(`${root}.dev-token`, 'utf8').trim();
const file = process.env.MELDFRAME_WEB_FILE;
if (!file) throw new Error('set MELDFRAME_WEB_FILE to a file name that is new for this run (see the top of this file)');
for (const u of [web, server]) {
  if (!['127.0.0.1', 'localhost', '[::1]'].includes(new URL(u).hostname)) throw new Error(`refusing to send the token to ${u}: the web and server URLs must be loopback (127.0.0.1, localhost or ::1)`);
}
const auth = { authorization: `Bearer ${token}` };
const run = Math.random().toString(36).slice(2, 7);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const pageErrors = [];
const results = [];
let n = 0;
let browser;
let page;

async function post(command) {
  const res = await fetch(`${server}/v1/files/${file}/tools/apply_commands`, { method: 'POST', headers: { 'content-type': 'application/json', ...auth }, body: JSON.stringify({ ops: [{ key: `iod-${run}-${++n}`, command, deps: [] }] }) });
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
  try {
    await fn();
    results.push(true);
    console.log(`ok   ${name}`);
  } catch (e) {
    results.push(false);
    console.log(`FAIL ${name}: ${String(e.message ?? e).split('\n')[0]}`);
  }
}
const $ = (sel) => page.locator(sel);
// Whether React drew the element: React puts a fibre on every DOM node it makes; a Svelte page has none.
const fibre = (pg, sel) => pg.evaluate((q) => Object.keys(document.querySelector(q) ?? {}).some((k) => k.startsWith('__reactFiber$')), sel);
const activeTestId = () => page.evaluate(() => document.activeElement?.getAttribute('data-testid') ?? '');
const inDialog = () => page.evaluate(() => !!document.activeElement?.closest('[role=dialog]'));
const bodyOverflow = () => page.evaluate(() => getComputedStyle(document.body).overflow);
const tool = () => page.getAttribute('[data-testid=tool]', 'data-tool');
// The two dialogs: the button that opens each, the dialog, its title and description, its Close button and a place that is not a control.
const DIALOGS = {
  export: { button: 'var-export', id: 'export-vars-dialog', title: 'Export variables', description: /Your file is not changed\./, close: 'export-vars-close' },
  import: { button: 'var-import', id: 'import-vars-dialog', title: 'Import variables', description: /one undo takes the import back\./, close: 'import-vars-close' },
};
const open = async (d) => {
  await $(`[data-testid=${d.button}]`).click();
  await $(`[data-testid=${d.id}]`).waitFor();
  await until('the focus is in the dialog', inDialog);
};
const closed = (d) => $(`[data-testid=${d.id}]`).waitFor({ state: 'detached' });
const closeBy = async (d, how) => {
  if (how === 'Escape') await page.keyboard.press('Escape');
  else if (how === 'Close') await $(`[data-testid=${d.close}]`).click();
  else {
    await page.getByRole('heading', { name: d.title }).click();
    assert.equal(await $(`[data-testid=${d.id}]`).count(), 1, 'a click inside keeps it open');
    await page.mouse.click(4, 4);
  }
  await closed(d);
};

(async () => {
  const seen = await fetch(`${server}/v1/files/${file}`, { headers: auth });
  if (seen.status !== 404) assert.equal(seen.status === 200 && (await seen.json()).seq, 0, 'the file has content already: start the page and the check with a file name that is new');
  browser = await chromium.launch({ args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
  const context = await browser.newContext({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1 });
  await context.grantPermissions(['clipboard-read', 'clipboard-write'], { origin: new URL(web).origin });
  page = await context.newPage();
  page.on('pageerror', (e) => { pageErrors.push(e.message); console.log('[pageerror]', e.message); });
  await page.goto(react ? `${web.replace(/\/?$/, '/')}?react=${words}` : web);
  await until('the editor is live', async () => (await page.getAttribute('[data-testid=connection]', 'data-state')) === 'live');

  await check(react ? 'the two dialogs are drawn by React' : 'the two dialogs are drawn by Svelte', async () => {
    for (const d of Object.values(DIALOGS)) {
      await open(d);
      assert.equal(await fibre(page, `[data-testid=${d.id}]`), react, react ? `${d.id} has no React fibre: the page fell back to Svelte` : `${d.id} has a React fibre on the Svelte run`);
      await closeBy(d, 'Escape');
    }
  });
  await check('each dialog has a name from its title and a description', async () => {
    const text = (id) => page.locator(`[id="${id}"]`).textContent();
    for (const d of Object.values(DIALOGS)) {
      await open(d);
      const dialog = $(`[data-testid=${d.id}]`);
      assert.equal(await dialog.getAttribute('role'), 'dialog');
      assert.equal((await text(await dialog.getAttribute('aria-labelledby'))).trim(), d.title);
      assert.match(await text(await dialog.getAttribute('aria-describedby')), d.description);
      await closeBy(d, 'Escape');
    }
  });
  await check('the first focus: Close in an export with nothing in it, the field in the import, never the Close icon', async () => {
    await open(DIALOGS.export);
    assert.equal(await activeTestId(), 'export-vars-done', 'with nothing to copy the focus starts on the Close button of the footer');
    await closeBy(DIALOGS.export, 'Escape');
    await open(DIALOGS.import);
    assert.equal(await activeTestId(), 'import-vars-text');
    await closeBy(DIALOGS.import, 'Escape');
  });
  await post({ type: 'addCollection', collection: { id: `c_${run}`, name: `brand-${run}`, modes: [{ id: `m_${run}`, name: 'default' }] } });
  await post({ type: 'addVariable', variable: { id: `v_${run}`, collection: `c_${run}`, name: 'color/ink', type: 'color', values: { [`m_${run}`]: '#112233' } } });
  await until('the variable is on the page', async () => /of 1$/.test((await $('[data-testid=var-count]').textContent()).trim()));
  await check('the first focus of an export that has text is Copy', async () => {
    await open(DIALOGS.export);
    assert.equal(await activeTestId(), 'export-vars-copy');
    await closeBy(DIALOGS.export, 'Escape');
  });
  await check('Tab and Shift+Tab keep the focus inside each dialog, also when pressed fast', async () => {
    for (const d of Object.values(DIALOGS)) {
      await open(d);
      for (const key of ['Tab', 'Shift+Tab']) {
        for (const times of [8, 20]) {
          for (let i = 0; i < times; i++) await page.keyboard.press(key);
          await sleep(250); // Base UI parks a focus guard for one tick at each end
          assert.equal(await inDialog(), true, `${d.id}: ${key} x${times} left the dialog`);
        }
      }
      await closeBy(d, 'Escape');
    }
  });
  await check('each dialog is above the canvas and the floating cards, the scrim covers the page, and the page behind does not scroll', async () => {
    for (const d of Object.values(DIALOGS)) {
      await open(d);
      const at = await page.evaluate((id) => {
        const r = document.querySelector(`[data-testid=${id}]`).getBoundingClientRect();
        const hit = (x, y) => document.elementFromPoint(x, y);
        return { centre: !!hit(r.x + r.width / 2, r.y + 20)?.closest(`[data-testid=${id}]`), corner: !!hit(4, 4)?.closest(`[data-testid=${id}]`), overflow: getComputedStyle(document.body).overflow };
      }, d.id);
      assert.deepEqual(at, { centre: true, corner: false, overflow: 'hidden' }, d.id);
      await closeBy(d, 'Escape');
    }
  });
  await check('Escape, the Close button and a click on the scrim each close each dialog and give the focus back to its button', async () => {
    for (const d of Object.values(DIALOGS)) {
      for (const how of ['Escape', 'Close', 'scrim']) {
        await open(d);
        await closeBy(d, how);
        await until(`${d.id}: the focus is on its button after ${how}`, async () => (await activeTestId()) === d.button);
        await until('the page scrolls again', async () => (await bodyOverflow()) !== 'hidden');
      }
    }
  });
  await check('what Copy said does not come back when the dialog is opened again', async () => {
    await open(DIALOGS.export);
    await $('[data-testid=export-vars-copy]').click();
    await until('copied', async () => (await $('[data-testid=export-vars-said]').textContent()) === 'Copied.');
    await closeBy(DIALOGS.export, 'Escape');
    await open(DIALOGS.export);
    assert.equal(await $('[data-testid=export-vars-said]').textContent(), '', 'Copied. came back');
    await closeBy(DIALOGS.export, 'Escape');
  });
  await check('the keys typed in a dialog are text or nothing: a tool letter typed in the import field stays in it, and does not change the tool', async () => {
    assert.equal(await tool(), 'select');
    await open(DIALOGS.import);
    await page.keyboard.type('rfvet');
    assert.equal(await $('[data-testid=import-vars-text]').inputValue(), 'rfvet');
    assert.equal(await tool(), 'select');
    await $('[data-testid=import-vars-clear]').click();
    await closeBy(DIALOGS.import, 'Escape');
    await open(DIALOGS.export);
    await page.keyboard.press('r');
    assert.equal(await tool(), 'select', 'a letter typed in the export dialog changed the tool');
    await closeBy(DIALOGS.export, 'Escape');
  });
  await check('no uncaught error in the page', async () => assert.deepEqual(pageErrors, []));
  await browser.close();
  const failed = results.filter((r) => !r).length;
  console.log(failed ? `${failed} of ${results.length} checks failed` : `all ${results.length} checks passed`);
  process.exit(failed ? 1 : 0);
})().catch(async (e) => {
  console.error(`io-dialog-e2e: ${e.message}`);
  await browser?.close().catch(() => {});
  process.exit(1);
});
