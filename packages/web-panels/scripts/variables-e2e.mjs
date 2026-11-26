// Browser check of the Variables panel against a real server: node packages/web-panels/scripts/variables-e2e.mjs [web-url] [server-url]
// The panel and the mode switch in a browser: a frame is bound to a variable, and switching the mode re-draws it on the canvas.
// Needs the development server (`just dev`, or the server binary with MELDFRAME_DEV_TOKENS and no DATABASE_URL, token in
// .dev-token at the root) and `just web` (Vite) running, the WASM bridge built, and Playwright from the global module
// path. It adds one frame, one collection and one variable to the file, and removes the variable and the collection at
// the end (the frame stays: deleting a frame needs the owner's approval): run it against the development server only.
// The React copies of the panel, the bind picker (Base UI's Popover) and its list (E7.3 slice S5) are checked by the same script with
// MELDFRAME_UI=react: it opens the page with /?react=variables, and a first check asks that React really drew the panel (a silent
// fallback to Svelte would pass every other check; the picker's popover is asked the same when it opens). Without it the Svelte copies
// (Bits UI) run. A last check with MELDFRAME_UI=react blocks the chunks of the React copies and asks that the Svelte panel and picker are
// drawn and work. The Layers panel's switch is /?react=layers (layers-e2e.mjs asks it).
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
// Whether React drew the element: React puts a fibre on every DOM node it makes; a Svelte page has none.
const fibre = (page, sel) => page.evaluate((q) => Object.keys(document.querySelector(q) ?? {}).some((k) => k.startsWith('__reactFiber$')), sel);
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
  await page.goto(react ? `${web.replace(/\/?$/, '/')}?react=variables` : web);
  await until('the editor is live', async () => (await page.getAttribute('[data-testid=connection]', 'data-state')) === 'live');
  await until('the frame is listed', async () => (await page.locator(`[data-testid=layer][data-id="${frame}"]`).count()) === 1);

  await check('the panel is drawn by the copy that was asked for', async () => {
    await until('the panel is drawn', async () => (await page.locator('[data-testid=variables]').count()) === 1); // the React side loads on demand
    assert.equal(await fibre(page, '[data-testid=variables]'), react, react ? 'the Variables panel has no React fibre: the page fell back to Svelte' : 'the Variables panel has a React fibre on the Svelte run');
  });

  await check('the panel is open at first load and lists the collection and the variable; the section folds', async () => {
    assert.equal(await page.getAttribute('[data-testid=toggle-variables]', 'aria-expanded'), 'true');
    await until('the row', async () => (await page.locator(`[data-testid=var-row][data-id="${bg}"]`).count()) === 1);
    assert.equal(await page.locator(`[data-testid=var-modes][data-id="${col}"] [role=radio]`).count(), 2);
    assert.equal(await page.inputValue(`[data-testid=var-row][data-id="${bg}"] [data-testid=var-value]`), LIGHT);
    // The list scrolls inside a capped box, so Properties stays on screen below it.
    const list = await page.locator('[data-testid=var-list]').boundingBox();
    assert.ok(list.height <= 192 + 1, `the list is capped: ${list.height}`);
    assert.ok(await page.locator('[data-testid=properties]').isVisible(), 'properties is on the page');
    await page.click('[data-testid=toggle-variables]');
    await until('the list folds away', async () => (await page.locator('[data-testid=variables]').count()) === 0);
    await page.click('[data-testid=toggle-variables]');
    await until('the list is back', async () => (await page.locator(`[data-testid=var-row][data-id="${bg}"]`).count()) === 1);
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

  // The bind picker (BindPicker and PickerList, Bits UI's Popover on Svelte and Base UI's on React) opens from the button at the end of the
  // Fill field and is open while the app state says so (the app layer raises `pickerChanged` for each change of it, so the page reads the
  // list again). A mouse click opens it, as the keyboard does: the button once toggled the popover twice for one click.
  const picker = page.locator('[data-testid=picker]');
  const trigger = page.locator('[data-testid=prop-fill-bind]');
  const active = () => page.evaluate(() => `${document.activeElement?.getAttribute('data-testid') ?? document.activeElement?.tagName}`);
  await check('the picker opens with the pointer or the keyboard, takes the focus into its search and lists the variables of the field', async () => {
    await trigger.click();
    await until('the rows', async () => (await page.locator('[data-testid=picker-row]').count()) >= 1);
    assert.equal(await trigger.getAttribute('aria-expanded'), 'true');
    assert.equal(await fibre(page, '[data-testid=picker]'), react, 'the picker is drawn by the copy that was asked for');
    assert.equal(await active(), 'picker-search');
    assert.ok(await page.locator('[data-testid=picker] ul[aria-label="Variables for Fill"]').count(), 'the list is named for the field');
    assert.equal(await page.getAttribute(`[data-testid=picker-row][data-id="${bg}"]`, 'aria-pressed'), 'false');
    assert.equal(await page.locator('[data-testid=picker-row][data-id="no-such-variable"]').count(), 0);
    await page.locator('[data-testid=picker-row]').first().focus();
    await page.keyboard.press('Escape');
    await until('Escape from a row closes it', async () => (await picker.count()) === 0);
    assert.equal(await active(), 'prop-fill-bind', 'the focus is back on the button');
    await trigger.focus();
    await page.keyboard.press('Enter');
    await until('the rows again, from the keyboard', async () => (await page.locator('[data-testid=picker-row]').count()) >= 1);
    assert.equal(await active(), 'picker-search');
    await page.locator('[data-testid=picker-row]').first().focus();
    await page.keyboard.press('Escape');
    await until('closed again', async () => (await picker.count()) === 0);
  });
  await check('the search narrows the rows and says so; the keys typed in it are text, not tools', async () => {
    await trigger.click();
    await until('the rows', async () => (await page.locator('[data-testid=picker-row]').count()) >= 1);
    const all = await page.locator('[data-testid=picker-row]').count();
    await page.fill('[data-testid=picker-search]', 'zzz');
    await until('no row', async () => (await page.locator('[data-testid=picker-row]').count()) === 0);
    assert.match(await page.textContent('[data-testid=picker-empty]'), /No variable matches the search\./);
    await page.fill('[data-testid=picker-search]', '');
    await page.press('[data-testid=picker-search]', 'r');
    await sleep(300);
    assert.equal(await page.inputValue('[data-testid=picker-search]'), 'r');
    assert.equal(await page.locator('[aria-pressed=true][aria-label*=ectangle]').count(), 0, 'r is text in the field, not the Rectangle tool');
    await page.fill('[data-testid=picker-search]', '');
    await until('all rows again', async () => (await page.locator('[data-testid=picker-row]').count()) === all);
  });
  await check('a click outside closes the picker and a row binds the field and closes it', async () => {
    await page.mouse.click(150, 400);
    await until('the click outside closes it', async () => (await picker.count()) === 0);
    await trigger.click();
    await until('the rows', async () => (await page.locator('[data-testid=picker-row]').count()) >= 1);
    await page.click(`[data-testid=picker-row][data-id="${bg}"]`);
    await until('the picker closes on a bind', async () => (await picker.count()) === 0);
    const node = await until('the binding on the server', async () => (await snapshot()).nodes.find((x) => x.id === frame && x.fill?.var === bg));
    assert.deepEqual(node.fill, { var: bg });
    await until('the chip of the variable', async () => (await page.locator('[data-testid=prop-fill-bound]').count()) === 1);
    await page.click('[data-testid=prop-fill-detach]');
    await until('the field is a plain field again', async () => (await trigger.count()) === 1);
  });
  await check('Tab stays inside the picker both ways, and the picker sits 8 px under its button, end aligned', async () => {
    await trigger.click();
    await until('the rows', async () => (await page.locator('[data-testid=picker-row]').count()) >= 1);
    await page.locator('[data-testid=picker-row]').last().focus();
    await page.keyboard.press('Tab');
    await sleep(250); // the focus guards of a library hold the focus for a tick
    assert.equal(await picker.count(), 1, 'Tab from the last row left the picker open');
    assert.equal(await active(), 'picker-search', 'Tab from the last row goes to the search field');
    await page.keyboard.press('Shift+Tab');
    await sleep(250);
    assert.equal(await picker.count(), 1, 'Shift+Tab from the search field left the picker open');
    // Where it lands is the library's own (Bits UI puts it on its content, the React copy on the last row): inside the picker is the rule.
    assert.equal(await page.evaluate(() => !!document.activeElement?.closest('[data-testid=picker]')), true, 'Shift+Tab from the search field leaves the picker');
    const t = await trigger.boundingBox();
    const b = await picker.boundingBox();
    assert.ok(Math.abs(b.y - (t.y + t.height + 8)) <= 1.5, `the picker is 8 px under the button: ${b.y - (t.y + t.height)}`);
    assert.ok(Math.abs(b.x + b.width - (t.x + t.width)) <= 1.5, `the picker is aligned to the end of the button: ${b.x + b.width - (t.x + t.width)}`);
    await page.locator('[data-testid=picker-row]').first().focus(); // Escape closes from a row; in the search field the Field keeps the key
    await page.keyboard.press('Escape');
    await until('closed', async () => (await picker.count()) === 0);
  });
  if (react) {
    // The popup of Base UI has the role of a dialog, so it needs a name (the Bits UI content has no role).
    await check('the popover is a dialog with a name', async () => {
      await trigger.click();
      await until('the rows', async () => (await page.locator('[data-testid=picker-row]').count()) >= 1);
      assert.equal(await picker.getAttribute('role'), 'dialog');
      assert.equal(await picker.getAttribute('aria-label'), 'Bind Fill to a variable');
      assert.equal(await page.getByRole('dialog', { name: 'Bind Fill to a variable' }).count(), 1, 'a reader finds the dialog by its name');
      await page.locator('[data-testid=picker-row]').first().focus();
      await page.keyboard.press('Escape');
      await until('closed', async () => (await picker.count()) === 0);
    });
    // Manage, Import variables and Export variables are buttons of the React panel that open Svelte dialogs, which the switch file hosts
    // beside the island. manage-e2e.mjs and io-e2e.mjs check those dialogs but not who drew the buttons: this asks that the React buttons open
    // them and that the focus comes back to the button that opened each.
    await check('through the React panel, Manage, Import variables and Export variables open their dialogs and the focus comes back to the button', async () => {
      assert.equal(await fibre(page, '[data-testid=var-manage]'), true, 'the Manage button is drawn by React');
      await page.fill('[data-testid=var-search]', 'zzz');
      await until('the search narrows the list', async () => (await page.textContent('[data-testid=var-count]')).trim().startsWith('0 of '));
      await page.click('[data-testid=var-manage]');
      await page.locator('[data-testid=manage-dialog]').waitFor();
      await until('the search was cleared before the view opened', async () => (await page.inputValue('[data-testid=var-search]')) === '');
      await page.keyboard.press('Escape');
      await page.locator('[data-testid=manage-dialog]').waitFor({ state: 'detached' });
      await until('the focus is back on Manage', async () => (await active()) === 'var-manage');
      await page.click('[data-testid=var-import]');
      await page.locator('[data-testid=import-vars-dialog]').waitFor();
      await page.keyboard.press('Escape');
      await page.locator('[data-testid=import-vars-dialog]').waitFor({ state: 'detached' });
      await until('the focus is back on Import variables', async () => (await active()) === 'var-import');
      await page.click('[data-testid=var-export]');
      await page.locator('[data-testid=export-vars-dialog]').waitFor();
      await page.keyboard.press('Escape');
      await page.locator('[data-testid=export-vars-dialog]').waitFor({ state: 'detached' });
      await until('the focus is back on Export variables', async () => (await active()) === 'var-export');
    });
  }

  if (react) {
    // The React copies load on demand. A chunk that does not load must leave the Svelte panel and the Svelte picker, never a page with no variables.
    await check('with the chunks of the React copies blocked, the Svelte panel and picker are drawn and work', async () => {
      const ctx = await browser.newContext({ viewport: { width: 1280, height: 800 } });
      const blocked = await ctx.newPage();
      blocked.on('pageerror', (e) => { pageErrors.push(e.message); console.log('[pageerror]', e.message); });
      await blocked.route((u) => /react\/(VariablesPanel|BindPicker|island)/.test(u.pathname), (route) => route.abort('failed'));
      await blocked.goto(`${web.replace(/\/?$/, '/')}?react=variables`);
      await until('the editor is live', async () => (await blocked.getAttribute('[data-testid=connection]', 'data-state')) === 'live');
      await until('the panel is drawn', async () => (await blocked.locator(`[data-testid=var-row][data-id="${bg}"]`).count()) === 1);
      assert.equal(await fibre(blocked, '[data-testid=variables]'), false, 'the panel is the Svelte copy');
      await blocked.locator(`[data-testid=layer][data-id="${frame}"] button`).last().click();
      await until('the bind button', async () => (await blocked.locator('[data-testid=prop-fill-bind]').count()) === 1);
      await blocked.locator('[data-testid=prop-fill-bind]').click();
      await until('the rows', async () => (await blocked.locator('[data-testid=picker-row]').count()) >= 1);
      assert.equal(await fibre(blocked, '[data-testid=picker]'), false, 'the picker is the Svelte copy');
      await blocked.keyboard.press('Escape');
      await blocked.locator('[data-testid=picker-row]').first().focus();
      await blocked.keyboard.press('Escape');
      await until('the picker closes', async () => (await blocked.locator('[data-testid=picker]').count()) === 0);
      await blocked.click('[data-testid=var-manage]');
      await blocked.locator('[role=dialog]').first().waitFor({ timeout: 5000 });
      await ctx.close();
    });
  }

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
