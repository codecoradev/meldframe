// Browser check of the shell of the Variables manage dialog against a real server: node packages/web-variables/scripts/manage-dialog-e2e.mjs [web-url] [server-url]
// What the dialog does as a dialog, whoever draws it (E7.3 slice S9): its name and description, where the focus starts, Tab and Shift+Tab, the scrim
// and the scroll lock, the three ways to close it and where the focus goes back to. manage-e2e.mjs checks what is done inside it.
// MELDFRAME_UI=react opens the page with /?react=manage (MELDFRAME_REACT_PANELS names other switch words, such as variables,manage) and a first check asks
// that React really drew the dialog (a silent fallback to Svelte would pass every other check); without it the Svelte dialog (Bits UI) runs. A last
// check with MELDFRAME_UI=react blocks the chunk of the React dialog and asks that the Svelte dialog is drawn and works.
// Start the server and the page as manage-e2e.mjs says (its header has the commands) with a file name that is new for every run; the token of the
// page is in .dev-token at the root, the web and server URLs must be loopback, the WASM bridge is built from this revision and Playwright comes from
// the global module path. It adds one collection to the file and leaves it: run it against a development server only.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';

const { chromium } = createRequire(import.meta.url)('playwright');
const react = process.env.MELDFRAME_UI === 'react';
const words = process.env.MELDFRAME_REACT_PANELS ?? 'manage';
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
let browser;
let page;

async function post(command) {
  const res = await fetch(`${server}/v1/files/${file}/tools/apply_commands`, { method: 'POST', headers: { 'content-type': 'application/json', ...auth }, body: JSON.stringify({ ops: [{ key: `md-${run}`, command, deps: [] }] }) });
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
const dialog = () => $('[data-testid=manage-dialog]');
// Whether React drew the element: React puts a fibre on every DOM node it makes; a Svelte page has none.
const fibre = (pg, sel) => pg.evaluate((q) => Object.keys(document.querySelector(q) ?? {}).some((k) => k.startsWith('__reactFiber$')), sel);
const activeTestId = () => page.evaluate(() => document.activeElement?.getAttribute('data-testid') ?? '');
const inDialog = () => page.evaluate(() => !!document.activeElement?.closest('[role=dialog]'));
const bodyOverflow = () => page.evaluate(() => getComputedStyle(document.body).overflow);
const open = async () => {
  await $('[data-testid=var-manage]').click();
  await dialog().waitFor();
  await until('the focus is in the dialog', inDialog);
};
const closed = () => dialog().waitFor({ state: 'detached' });
const closeBy = async (how) => {
  if (how === 'Escape') {
    // Not from a field or the question: a Field keeps Escape for itself, and the question takes it to cancel only itself.
    await $('[data-testid=manage-collections-title]').focus();
    await page.keyboard.press('Escape');
  } else if (how === 'Close') await $('[data-testid=manage-close]').click();
  else {
    await $('[data-testid=manage-new-collection]').click();
    assert.equal(await dialog().count(), 1, 'a click inside keeps it open');
    await page.mouse.click(4, 4);
  }
  await closed();
};

(async () => {
  const seen = await fetch(`${server}/v1/files/${file}`, { headers: auth });
  if (seen.status !== 404) assert.equal(seen.status === 200 && (await seen.json()).seq, 0, 'the file has content already: start the page and the check with a file name that is new');
  browser = await chromium.launch({ args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
  page = await browser.newPage({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1 });
  page.on('pageerror', (e) => { pageErrors.push(e.message); console.log('[pageerror]', e.message); });
  await page.goto(react ? `${web.replace(/\/?$/, '/')}?react=${words}` : web);
  await until('the editor is live', async () => (await page.getAttribute('[data-testid=connection]', 'data-state')) === 'live');

  await check(react ? 'the dialog is drawn by React' : 'the dialog is drawn by Svelte', async () => {
    await open();
    assert.equal(await fibre(page, '[data-testid=manage-dialog]'), react, react ? 'the dialog has no React fibre: the page fell back to Svelte' : 'the dialog has a React fibre on the Svelte run');
  });
  await check('the dialog has a name from its title and a description, and the first focus is the field of the new collection when there is none', async () => {
    const text = (id) => page.locator(`[id="${id}"]`).textContent();
    assert.equal(await dialog().getAttribute('role'), 'dialog');
    assert.equal((await text(await dialog().getAttribute('aria-labelledby'))).trim(), 'Manage variables');
    assert.match(await text(await dialog().getAttribute('aria-describedby')), /A removal asks first, and one undo takes it back\./);
    assert.equal(await activeTestId(), 'manage-new-collection', 'the focus starts on the field, not on the Close button');
  });
  await check('Tab and Shift+Tab keep the focus inside the dialog, also when pressed fast', async () => {
    for (const key of ['Tab', 'Shift+Tab']) {
      for (const n of [8, 20]) {
        for (let i = 0; i < n; i++) await page.keyboard.press(key);
        await sleep(250); // Base UI parks a focus guard for one tick at each end
        assert.equal(await inDialog(), true, `${key} x${n} left the dialog`);
      }
    }
  });
  await check('the dialog is above the canvas and the floating cards, the scrim covers the page, and the page behind does not scroll', async () => {
    const at = await page.evaluate(() => {
      const d = document.querySelector('[data-testid=manage-dialog]').getBoundingClientRect();
      const hit = (x, y) => document.elementFromPoint(x, y);
      return { centre: !!hit(d.x + d.width / 2, d.y + 20)?.closest('[data-testid=manage-dialog]'), corner: !!hit(4, 4)?.closest('[data-testid=manage-dialog]'), overflow: getComputedStyle(document.body).overflow };
    });
    assert.deepEqual(at, { centre: true, corner: false, overflow: 'hidden' });
  });
  await check('Escape, the Close button and a click on the scrim each close it and give the focus back to the Manage button', async () => {
    for (const how of ['Escape', 'Close', 'scrim']) {
      await closeBy(how);
      await until(`the focus is on Manage after ${how}`, async () => (await activeTestId()) === 'var-manage');
      await until('the page scrolls again', async () => (await bodyOverflow()) !== 'hidden');
      await open();
    }
  });
  // The dialog is drawn beside the Export dialog and does not live in the Variables panel: Option+2 folds the right card (an Alt key is not owned
  // by a dialog), and the dialog stays; a dialog that went with its card left `manage.open` set, so Ctrl+K was refused with nothing on the screen.
  await check('Option+2 with the dialog open folds the right card and leaves the dialog; Ctrl+K opens nothing over it; closed, the keys work again', async () => {
    if ((await dialog().count()) === 0) await open(); // the check before leaves it open
    await $('[data-testid=manage-close]').focus(); // a field keeps every key, Alt ones too; a button of the dialog keeps only the plain keys
    await page.keyboard.press('Alt+Digit2');
    await until('the right card is folded', async () => (await $('[data-testid=var-manage]').count()) === 0);
    assert.equal(await dialog().count(), 1, 'the dialog went with the right card');
    await page.keyboard.press('Control+KeyK');
    await sleep(300);
    assert.equal(await $('[data-testid=palette]').count(), 0, 'the palette opened over the dialog');
    assert.equal(await dialog().count(), 1, 'Ctrl+K took the dialog away');
    await closeBy('Escape');
    await page.keyboard.press('Control+KeyK');
    await until('the palette opens once the dialog is closed', async () => (await $('[data-testid=palette]').count()) === 1);
    await page.keyboard.press('Escape');
    await until('the palette is closed', async () => (await $('[data-testid=palette]').count()) === 0);
    await page.keyboard.press('Alt+Digit2');
    await until('the right card is back', async () => (await $('[data-testid=var-manage]').count()) === 1);
    await open(); // the next check starts with the dialog open, as the one before this left it
  });
  await check('opened again in a file that has a collection, the first focus is the collection on view', async () => {
    await $('[data-testid=manage-collections-title]').focus(); // not a field: a Field keeps Escape for itself
    await page.keyboard.press('Escape');
    await closed();
    await post({ type: 'addCollection', collection: { id: `c_${run}`, name: `brand-${run}`, modes: [{ id: `m_${run}`, name: 'default' }] } });
    await until('the collection is on the page', async () => (await $('[data-testid=var-collection]').count()) === 1);
    await open();
    assert.equal(await activeTestId(), 'manage-collection');
    assert.equal(await page.evaluate(() => document.activeElement?.getAttribute('aria-current')), 'true');
    await page.keyboard.press('Escape');
    await closed();
  });
  await check('whichever way it closes, the app layer is told: an open rename field and an open question do not come back', async () => {
    for (const how of ['Escape', 'Close', 'scrim']) {
      for (const what of ['rename field', 'question']) {
        await open();
        if (what === 'question') await $('[data-testid=manage-remove-collection]').click();
        else await $('[data-rename^="collection:"]').click();
        await $(what === 'question' ? '[data-testid=remove-confirm]' : '[data-testid=manage-renaming-collection]').waitFor();
        await closeBy(how);
        await until(`the focus is on Manage after ${how}`, async () => (await activeTestId()) === 'var-manage');
        await open();
        assert.equal(await $('[data-testid=manage-renaming-collection]').count(), 0, `the rename field came back after ${how}`);
        assert.equal(await $('[data-testid=remove-confirm]').count(), 0, `the question came back after ${how}`);
        await closeBy('Escape');
      }
    }
  });
  if (react) {
    // The React dialog loads on demand. A chunk that does not load must leave the Svelte dialog, never a Manage button that opens nothing.
    await check('with the chunk of the React dialog blocked, the Svelte dialog is drawn and works', async () => {
      const ctx = await browser.newContext({ viewport: { width: 1280, height: 800 } });
      const blocked = await ctx.newPage();
      blocked.on('pageerror', (e) => { pageErrors.push(e.message); console.log('[pageerror]', e.message); });
      await blocked.route((u) => /react\/ManageDialog/.test(u.pathname), (route) => route.abort('failed'));
      await blocked.goto(`${web.replace(/\/?$/, '/')}?react=manage`);
      await blocked.waitForSelector('[data-testid=connection][data-state=live]');
      await blocked.click('[data-testid=var-manage]');
      await blocked.locator('[data-testid=manage-dialog]').waitFor({ timeout: 5000 });
      assert.equal(await fibre(blocked, '[data-testid=manage-dialog]'), false, 'the dialog is the Svelte copy');
      assert.equal(await blocked.locator('[data-testid=manage-collection]').count(), 1, 'it lists the collection of the file');
      await blocked.keyboard.press('Escape');
      await blocked.locator('[data-testid=manage-dialog]').waitFor({ state: 'detached' });
      await ctx.close();
    });
  }
  await check('no uncaught error in the page', async () => assert.deepEqual(pageErrors, []));
  await browser.close();
  const failed = results.filter((r) => !r).length;
  console.log(failed ? `${failed} of ${results.length} checks failed` : `all ${results.length} checks passed`);
  process.exit(failed ? 1 : 0);
})().catch(async (e) => {
  console.error(`manage-dialog-e2e: ${e.message}`);
  await browser?.close().catch(() => {});
  process.exit(1);
});
