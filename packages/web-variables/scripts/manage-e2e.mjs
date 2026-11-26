// Browser check of the Variables manage view against a real server: node packages/web-variables/scripts/manage-e2e.mjs [web-url] [server-url]
// The manage view in a browser (D8): add a collection, a mode (copying a mode) and variables, the checks of a name before
// anything is sent, rename a variable, a mode and a collection (one op, one undo, the ids and bindings stay, the focus
// back on the Rename button, Escape cancels only the edit), the confirmation of a removal with Detach and Cancel, one
// undo of a detach, and the focus going where the person is. Start the server and the page with a file name that is new for every run (the check refuses a file
// that has content), the server on any free port (here 18080) and the token of the page in .dev-token at the root:
//   MELDFRAME_DEV_TOKENS="$(cat .dev-token):ws_dev:agent:dev" MELDFRAME_LISTEN=127.0.0.1:18080 <server binary or just dev-server>
//   MELDFRAME_WEB_FILE=<file> MELDFRAME_WEB_TOKEN="$(cat .dev-token)" MELDFRAME_SERVER=http://127.0.0.1:18080 npx vite --port 15173   (in apps/web)
//   MELDFRAME_WEB_FILE=<file> node packages/web-variables/scripts/manage-e2e.mjs http://localhost:15173 http://127.0.0.1:18080
// The WASM bridge must be built from this revision (`just wasm-build`: the six manage actions and the rename commands of EditVariables are in it) and Playwright
// comes from the global module path. The web and server URLs must be loopback (the token is sent there). It adds a
// frame, collections and variables to the file and leaves them: run it against a development server only.
// The checks run in order and share the file: some start from what an earlier one left (the renamed variable and its undo, the
// collection of the run). A check that fails can fail the ones after it.
// MELDFRAME_E2E_SHOTS=<dir> saves screenshots (and one of the page when a check fails).
// MELDFRAME_UI=react opens /?react=manage (MELDFRAME_REACT_PANELS names other switch words, such as variables,manage): the React dialog, E7.3 slice S9; the first check asks that React drew it. manage-dialog-e2e.mjs checks the dialog's shell on both.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';

const { chromium } = createRequire(import.meta.url)('playwright');
const react = process.env.MELDFRAME_UI === 'react';
const web = process.argv[2] ?? 'http://localhost:5173';
const server = process.argv[3] ?? 'http://127.0.0.1:8080';
const root = new URL('../../../', import.meta.url).pathname;
const token = readFileSync(`${root}.dev-token`, 'utf8').trim();
const file = process.env.MELDFRAME_WEB_FILE;
const shots = process.env.MELDFRAME_E2E_SHOTS;
if (!file) throw new Error('set MELDFRAME_WEB_FILE to a file name that is new for this run (see the top of this file)');
for (const u of [web, server]) {
  if (!['127.0.0.1', 'localhost', '[::1]'].includes(new URL(u).hostname)) throw new Error(`refusing to send the token to ${u}: the web and server URLs must be loopback (127.0.0.1, localhost or ::1)`);
}
const run = Math.random().toString(36).slice(2, 7);
const auth = { authorization: `Bearer ${token}` };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const pageErrors = [];
let n = 0;
let browser;
let page;

async function post(command) {
  const res = await fetch(`${server}/v1/files/${file}/tools/apply_commands`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', ...auth },
    body: JSON.stringify({ ops: [{ key: `mg-${run}-${++n}`, command, deps: [] }] }),
  });
  assert.equal(res.status, 200, `op refused: ${res.status} ${await res.text()}`);
}
const snapshot = async () => {
  const res = await fetch(`${server}/v1/files/${file}`, { headers: auth });
  return res.status === 200 ? res.json() : { seq: 0, nodes: [], variables: { collections: [], variables: [] } };
};
const collections = async () => (await snapshot()).variables?.collections ?? [];
const variables = async () => (await snapshot()).variables?.variables ?? [];
async function until(what, fn, ms = 15000) {
  const t0 = Date.now();
  for (;;) {
    const v = await fn();
    if (v) return v;
    if (Date.now() - t0 > ms) throw new Error(`timed out waiting for: ${what}`);
    await sleep(150);
  }
}
const $ = (sel) => page.locator(sel);
const text = async (sel) => (await $(sel).first().textContent())?.trim() ?? '';
const activeTestId = () => page.evaluate(() => document.activeElement?.getAttribute('data-testid') ?? '');
const activeId = () => page.evaluate(() => document.activeElement?.getAttribute('data-id') ?? document.activeElement?.getAttribute('data-entity-id') ?? '');
const results = [];
async function check(name, fn) {
  try {
    await fn();
    results.push(true);
    console.log(`ok   ${name}`);
  } catch (e) {
    results.push(false);
    if (shots && page) await page.screenshot({ path: `${shots}/manage-fail-${results.length}.png` }).catch(() => {});
    console.log(`FAIL ${name}: ${String(e.message ?? e).split('\n').slice(0, process.env.MELDFRAME_E2E_DEBUG ? 12 : 1).join(' | ')}`);
  }
}
const shot = (name) => (shots && page ? page.screenshot({ path: `${shots}/manage-${name}.png` }) : null);

const brand = `brand-${run}`;
const inkName = `color/ink-${run}`;
const gapName = `space/gap-${run}`;
const frame = `frame_${run}`;
const addEntity = async (fieldId, value) => {
  await $(`[data-testid=${fieldId}]`).fill(value);
  await $(`[data-testid=${fieldId}]`).press('Enter');
};

(async () => {
  assert.equal((await snapshot()).seq, 0, 'the file has content already: start the page and the check with a file name that is new');
  await post({ type: 'addNode', node: { id: frame, name: `Card ${run}`, type: 'frame', parentId: null, x: 0, y: 0, w: 300, h: 200, fill: '#ffffff' } });
  browser = await chromium.launch({ args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
  page = await browser.newPage({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1 });
  page.on('pageerror', (e) => { pageErrors.push(e.message); console.log('[pageerror]', e.message); });
  await page.goto(react ? `${web.replace(/\/?$/, '/')}?react=${process.env.MELDFRAME_REACT_PANELS ?? 'manage'}` : web);
  await until('the editor is live', async () => (await page.getAttribute('[data-testid=connection]', 'data-state')) === 'live');
  await until('the frame is listed', async () => (await $(`[data-testid=layer][data-id="${frame}"]`).count()) === 1);

  await check('the Manage button opens the view, drawn by the copy that was asked for, with the focus inside it and the scrim over the page', async () => {
    await $('[data-testid=var-manage]').click();
    await $('[data-testid=manage-dialog]').waitFor();
    assert.equal(await page.evaluate(() => Object.keys(document.querySelector('[data-testid=manage-dialog]')).some((k) => k.startsWith('__reactFiber$'))), react, 'the dialog is not drawn by the copy that was asked for (a fallback to Svelte?)');
    await until('the focus is in the dialog', () => page.evaluate(() => document.querySelector('[data-testid=manage-dialog]')?.contains(document.activeElement)));
    assert.equal(await $('[data-testid=manage-collection]').count(), 0, 'a new file has no collection');
    assert.match(await text('[data-testid=manage-no-collections]'), /no collections yet/i);
    await shot('empty');
  });

  await check('the name of a collection is checked before anything is sent, in the rule\'s own words', async () => {
    const seq = (await snapshot()).seq;
    const field = $('[data-testid=manage-new-collection]');
    await field.fill('Brand Name');
    assert.match(await text('[data-testid=manage-new-collection-note]'), /lowercase letters, digits and single hyphens/i);
    assert.equal(await $('[data-testid=manage-new-collection-add]').isDisabled(), true);
    await field.press('Enter');
    await field.fill('dark‮x');
    assert.match(await text('[data-testid=manage-new-collection-note]'), /control, direction or invisible/i);
    await field.fill('');
    assert.equal(await $('[data-testid=manage-new-collection-add]').isDisabled(), true);
    await sleep(300);
    assert.equal((await snapshot()).seq, seq, 'nothing was sent');
  });

  await check('adding a collection with Enter sends one op, selects it, focuses it and starts the form again', async () => {
    await addEntity('manage-new-collection', brand);
    const c = await until('the collection on the server', async () => (await collections()).find((x) => x.name === brand));
    assert.deepEqual(c.modes.map((m) => m.name), ['default']);
    const row = $(`[data-testid=manage-collection][data-id="${c.id}"]`);
    await row.waitFor();
    assert.equal(await row.getAttribute('aria-current'), 'true');
    await until('the focus is on the new collection', async () => (await activeId()) === c.id);
    assert.equal(await $('[data-testid=manage-new-collection]').inputValue(), '');
    assert.match(await text('[data-testid=manage-status]'), new RegExp(`Added the collection “${brand}”`));
    assert.equal(await text('[data-testid=manage-title]'), brand);
  });

  await check('a mode is added after the first, copying the one picked; a name that exists is a warning, not a refusal', async () => {
    const c = (await collections()).find((x) => x.name === brand);
    await addEntity('manage-new-mode', 'dark');
    const grown = await until('two modes on the server', async () => (await collections()).find((x) => x.id === c.id && x.modes.length === 2));
    assert.deepEqual(grown.modes.map((m) => m.name), ['default', 'dark']);
    const mode = grown.modes[1];
    await until('the focus is on the new mode', async () => (await activeId()) === mode.id);
    assert.equal(await $('[data-testid=manage-mode]').count(), 2);
    // The same name again: the core takes it, the view says so and does not stop the person.
    await $('[data-testid=manage-new-mode]').fill('Dark');
    assert.equal(await $('[data-testid=manage-new-mode-note]').getAttribute('data-tone'), 'warning');
    assert.match(await text('[data-testid=manage-new-mode-note]'), /already exists/i);
    assert.equal(await $('[data-testid=manage-new-mode-add]').isDisabled(), false);
    // A name that is drawn as nothing in part could pass for the badge of the default mode: refused in the field.
    await $('[data-testid=manage-new-mode]').fill('dar\u3164k');
    assert.match(await text('[data-testid=manage-new-mode-note]'), /invisible/i);
    assert.equal(await $('[data-testid=manage-new-mode-add]').isDisabled(), true);
    await $('[data-testid=manage-new-mode]').fill('');
    assert.equal(await $('[data-testid=manage-remove-mode]').count(), 1, 'the default mode has no Remove');
  });

  await check('variables are added with their type; the focus goes to the new row and the table shows type and uses', async () => {
    await $('[data-testid=manage-new-type] [role=radio]:has-text("Number")').click();
    await addEntity('manage-new-variable', gapName);
    const gap = await until('the number on the server', async () => (await variables()).find((v) => v.name === gapName));
    assert.equal(gap.type, 'number');
    await until('the focus is on the row', async () => (await activeId()) === gap.id);
    await $('[data-testid=manage-new-type] [role=radio]:has-text("Color")').click();
    await addEntity('manage-new-variable', inkName);
    const ink = await until('the colour on the server', async () => (await variables()).find((v) => v.name === inkName));
    assert.equal(ink.type, 'color');
    assert.deepEqual(Object.keys(ink.values).length, 2, 'a value in each mode');
    const rowInk = $(`[data-testid=manage-var][data-id="${ink.id}"]`);
    assert.equal(await rowInk.locator('[data-testid=manage-var-type]').textContent(), 'Color');
    assert.equal(await rowInk.locator('[data-testid=manage-var-uses]').textContent(), '0');
    assert.match(await text('[data-testid=manage-summary]'), /2 variables · 0 uses · 2 modes/);
    // A variable of that name is there: the core would refuse it, the view does not send it.
    const seq = (await snapshot()).seq;
    await $('[data-testid=manage-new-variable]').fill(inkName);
    assert.match(await text('[data-testid=manage-new-variable-note]'), /already has this name/i);
    assert.equal(await $('[data-testid=manage-new-variable-add]').isDisabled(), true);
    await $('[data-testid=manage-new-variable]').press('Enter');
    await $('[data-testid=manage-new-variable]').fill('');
    await sleep(300);
    assert.equal((await snapshot()).seq, seq);
    await shot('table');
  });

  await check('a use that arrives from outside shows in the table at once, and the question says how many; a question for another row takes the focus again', async () => {
    const ink = (await variables()).find((v) => v.name === inkName);
    await post({ type: 'updateNode', id: frame, patch: { fill: { var: ink.id } } });
    const row = $(`[data-testid=manage-var][data-id="${ink.id}"]`);
    await until('one use', async () => (await row.locator('[data-testid=manage-var-uses]').textContent()) === '1');
    assert.match(await text('[data-testid=manage-summary]'), /1 use\b/);
    await row.locator('[data-testid=manage-remove-variable]').click();
    const q = $('[data-testid=remove-confirm]');
    await q.waitFor();
    assert.match(await q.textContent(), /has 1 use/);
    assert.equal((await q.locator('[data-testid=remove-confirm-yes]').textContent()).trim(), 'Detach');
    await until('the safe button has the focus', async () => (await activeTestId()) === 'remove-cancel');
    await shot('confirm');
    // A question for another row starts again: it names that row and takes the focus on Cancel as the first one did.
    const gap = (await variables()).find((v) => v.name === gapName);
    await $(`[data-testid=manage-var][data-id="${gap.id}"] [data-testid=manage-remove-variable]`).click();
    assert.ok((await text('[data-testid=remove-confirm]')).includes(`“${gapName}”`), 'the question names the other row');
    await until('Cancel has the focus again', async () => (await activeTestId()) === 'remove-cancel');
    await $(`[data-testid=manage-var][data-id="${ink.id}"] [data-testid=manage-remove-variable]`).click();
    assert.match(await text('[data-testid=remove-confirm]'), /has 1 use/);
    await until('Cancel has the focus once more', async () => (await activeTestId()) === 'remove-cancel');
  });

  await check('Escape cancels the question only; Cancel changes nothing and gives the focus back to the button that asked', async () => {
    const ink = (await variables()).find((v) => v.name === inkName);
    const seq = (await snapshot()).seq;
    await page.keyboard.press('Escape');
    await until('the question is gone', async () => (await $('[data-testid=remove-confirm]').count()) === 0);
    assert.equal(await $('[data-testid=manage-dialog]').count(), 1, 'the dialog is still open');
    assert.equal(await activeTestId(), 'manage-remove-variable');
    await $(`[data-testid=manage-var][data-id="${ink.id}"] [data-testid=manage-remove-variable]`).click();
    await $('[data-testid=remove-cancel]').click();
    await until('the question is gone', async () => (await $('[data-testid=remove-confirm]').count()) === 0);
    assert.equal(await activeTestId(), 'manage-remove-variable');
    await sleep(300);
    const after = await snapshot();
    assert.equal(after.seq, seq, 'no op was sent');
    assert.deepEqual(after.nodes.find((x) => x.id === frame).fill, { var: ink.id });
  });

  // Renaming (D8): one rename command in one EditVariables action, one undo step, the ids stay.
  const renameKey = (key) => $(`[data-rename="${key}"]`);
  const activeRename = () => page.evaluate(() => document.activeElement?.getAttribute('data-rename') ?? '');
  const dialogOpen = async () => (await $('[data-testid=manage-dialog]').count()) === 1;
  const inkId = async () => (await variables()).find((v) => v.name === inkName || v.name === `${inkName}-b`).id;

  await check('renaming a variable: the button names its target, the field is prefilled, Enter sends one rename, the row and the status say it and the focus goes back', async () => {
    const ink = (await variables()).find((v) => v.name === inkName);
    const seq = (await snapshot()).seq;
    const btn = renameKey(`variable:${ink.id}`);
    assert.equal(await btn.getAttribute('aria-label'), `Rename ${inkName}`);
    assert.equal((await btn.textContent()).trim(), 'Rename');
    await btn.click();
    const f = $('[data-testid=manage-renaming-variable]');
    await f.waitFor();
    assert.equal(await f.inputValue(), inkName, 'prefilled with the current name');
    assert.equal(await f.getAttribute('aria-label'), `New name of ${inkName}`);
    await until('the field has the focus', async () => (await activeTestId()) === 'manage-renaming-variable');
    assert.equal(await renameKey(`variable:${ink.id}`).count(), 0, 'no Rename button for the row while its field is open');
    assert.match(await text('[data-testid=manage-renaming-variable-note]'), /next export uses the new name/i);
    assert.match(await text('[data-testid=manage-renaming-variable-note]'), /bindings and aliases keep working/i);
    await shot('rename-open');
    await f.fill(`${inkName}-b`);
    await f.press('Enter');
    const renamed = await until('the new name on the server', async () => (await variables()).find((v) => v.id === ink.id && v.name === `${inkName}-b`));
    assert.equal(renamed.id, ink.id);
    await until('the field is gone', async () => (await $('[data-testid=manage-renaming-variable]').count()) === 0);
    assert.equal((await snapshot()).seq, seq + 1, 'one op');
    assert.equal(await text(`[data-testid=manage-var][data-id="${ink.id}"] th`), `${inkName}-b`);
    assert.match(await text('[data-testid=manage-status]'), new RegExp(`Renamed “${inkName}” to “${inkName}-b”\\.`));
    await until('the focus is back on its Rename button', async () => (await activeRename()) === `variable:${ink.id}`);
    assert.equal(await renameKey(`variable:${ink.id}`).getAttribute('aria-label'), `Rename ${inkName}-b`);
    assert.equal(await $(`[data-testid=manage-var][data-id="${ink.id}"] [data-testid=manage-var-uses]`).textContent(), '1', 'its use stays');
    assert.deepEqual((await snapshot()).nodes.find((x) => x.id === frame).fill, { var: ink.id }, 'the binding keeps the id');
    await shot('rename-done');
  });

  await check('a bound node keeps showing the variable under its new name in the Variables panel; one undo takes the rename back', async () => {
    const id = await inkId();
    const seq = (await snapshot()).seq;
    await page.keyboard.press('Escape');
    await until('the dialog is gone', async () => !(await dialogOpen()));
    await page.click(`[data-testid=layer][data-id="${frame}"] button >> nth=-1`);
    const prow = $(`[data-testid=var-row][data-id="${id}"]`);
    await until('the panel row has the new name', async () => (await prow.count()) === 1 && (await prow.textContent()).includes(`${inkName}-b`));
    assert.equal(await prow.locator('[data-testid=var-bind][data-field=fill]').getAttribute('aria-pressed'), 'true', 'the frame is still bound to it');
    await $('[data-testid=var-manage]').click();
    await $('[data-testid=manage-dialog]').waitFor();
    await page.keyboard.press('Control+z');
    await until('the old name is back', async () => (await variables()).some((v) => v.id === id && v.name === inkName));
    assert.equal((await snapshot()).seq, seq + 1, 'the undo is one op');
    await until('the row shows the old name', async () => (await text(`[data-testid=manage-var][data-id="${id}"] th`)) === inkName);
    assert.deepEqual((await snapshot()).nodes.find((x) => x.id === frame).fill, { var: id });
  });

  await check('a duplicate variable name is refused in the field with its words and nothing is sent; Escape cancels only the edit; an unchanged name is a quiet cancel', async () => {
    const ink = (await variables()).find((v) => v.name === inkName);
    const seq = (await snapshot()).seq;
    await renameKey(`variable:${ink.id}`).click();
    const f = $('[data-testid=manage-renaming-variable]');
    await f.fill(gapName);
    const note = $('[data-testid=manage-renaming-variable-note]');
    assert.equal(await note.getAttribute('data-tone'), 'error');
    assert.match(await note.textContent(), /already has this name/i);
    assert.equal(await $('[data-testid=manage-renaming-variable-save]').isDisabled(), true);
    await f.press('Enter');
    await sleep(300);
    assert.equal(await f.count(), 1, 'the field stays open for a correction');
    assert.equal((await snapshot()).seq, seq, 'nothing was sent');
    await f.fill('Color/Ink');
    assert.match(await note.textContent(), /lowercase letters/i);
    await f.fill(inkName);
    assert.equal(await $('[data-testid=manage-renaming-variable-save]').isDisabled(), false, 'its own name is not taken');
    // Escape cancels the edit only: the dialog stays, the name stays, the focus returns to the button.
    await f.fill(`${inkName}-zz`);
    await page.keyboard.press('Escape');
    await until('the field is gone', async () => (await $('[data-testid=manage-renaming-variable]').count()) === 0);
    assert.equal(await dialogOpen(), true, 'the dialog is still open');
    assert.equal(await activeRename(), `variable:${ink.id}`);
    assert.equal(await text(`[data-testid=manage-var][data-id="${ink.id}"] th`), inkName);
    // The name it has now, submitted: closes the field and sends nothing.
    await renameKey(`variable:${ink.id}`).click();
    await $('[data-testid=manage-renaming-variable]').press('Enter');
    await until('the field is gone', async () => (await $('[data-testid=manage-renaming-variable]').count()) === 0);
    assert.equal(await activeRename(), `variable:${ink.id}`);
    await renameKey(`variable:${ink.id}`).click();
    await $('[data-testid=manage-renaming-variable-save]').click();
    await until('the field is gone', async () => (await $('[data-testid=manage-renaming-variable]').count()) === 0);
    await sleep(300);
    assert.equal((await snapshot()).seq, seq, 'no op was sent by any of it');
  });

  await check('renaming a mode: the chip takes a field, a duplicate is a warning, hidden characters and 65 characters are refused, Enter renames, one undo', async () => {
    const c = (await collections()).find((x) => x.name === brand);
    const [first, second] = c.modes;
    assert.equal(await $('[data-testid=manage-rename-mode]').count(), 2, 'the default mode can be renamed too');
    const seq = (await snapshot()).seq;
    const btn = renameKey(`mode:${second.id}`);
    assert.equal(await btn.getAttribute('aria-label'), 'Rename mode dark');
    await btn.click();
    const f = $('[data-testid=manage-renaming-mode]');
    assert.equal(await f.inputValue(), 'dark');
    const note = $('[data-testid=manage-renaming-mode-note]');
    await f.fill('Default');
    assert.equal(await note.getAttribute('data-tone'), 'warning');
    assert.equal(await $('[data-testid=manage-renaming-mode-save]').isDisabled(), false);
    await f.fill('niㅤght');
    assert.match(await note.textContent(), /invisible/i);
    assert.equal(await $('[data-testid=manage-renaming-mode-save]').isDisabled(), true);
    await f.fill('x'.repeat(65));
    assert.match(await note.textContent(), /64 characters/);
    await f.press('Enter');
    await sleep(300);
    assert.equal((await snapshot()).seq, seq, 'nothing was sent');
    await f.fill('Night shift');
    await f.press('Enter');
    await until('the mode is renamed on the server', async () => (await collections()).find((x) => x.id === c.id)?.modes[1]?.name === 'Night shift');
    assert.equal((await collections()).find((x) => x.id === c.id).modes[1].id, second.id, 'same id');
    assert.equal((await snapshot()).seq, seq + 1);
    await until('the chip shows it', async () => (await text(`[data-testid=manage-mode][data-id="${second.id}"]`)).includes('Night shift'));
    assert.match(await text('[data-testid=manage-status]'), /Renamed the mode “dark” to “Night shift”\./);
    await until('the focus is back on the Rename button of the chip', async () => (await activeRename()) === `mode:${second.id}`);
    await page.keyboard.press('Control+z');
    await until('the old name is back', async () => (await collections()).find((x) => x.id === c.id)?.modes[1]?.name === 'dark');
    assert.equal((await snapshot()).seq, seq + 2, 'one undo step');
    assert.equal(first.name, 'default');
  });

  await check('renaming a collection from its title: the field takes the title, Enter renames, the list and the title follow, one undo; Escape keeps the name', async () => {
    const c = (await collections()).find((x) => x.name === brand);
    const seq = (await snapshot()).seq;
    const btn = renameKey(`collection:${c.id}`);
    assert.equal(await btn.getAttribute('aria-label'), `Rename collection ${brand}`);
    await btn.click();
    assert.equal(await $('[data-testid=manage-title]').count(), 0, 'the field is in place of the title');
    const f = $('[data-testid=manage-renaming-collection]');
    assert.equal(await f.inputValue(), brand);
    await f.fill('Brand Name');
    assert.match(await text('[data-testid=manage-renaming-collection-note]'), /lowercase letters/i);
    await page.keyboard.press('Escape');
    await until('the field is gone', async () => (await $('[data-testid=manage-renaming-collection]').count()) === 0);
    assert.equal(await dialogOpen(), true);
    assert.equal(await activeRename(), `collection:${c.id}`);
    assert.equal(await text('[data-testid=manage-title]'), brand);
    await btn.click();
    await f.fill(`${brand}-2`);
    await f.press('Enter');
    await until('the collection is renamed on the server', async () => (await collections()).some((x) => x.id === c.id && x.name === `${brand}-2`));
    assert.equal((await snapshot()).seq, seq + 1);
    await until('the title shows it', async () => (await text('[data-testid=manage-title]')) === `${brand}-2`);
    assert.equal((await text(`[data-testid=manage-collection][data-id="${c.id}"]`)).startsWith(`${brand}-2`), true, 'the list row too');
    assert.match(await text('[data-testid=manage-status]'), new RegExp(`Renamed the collection “${brand}” to “${brand}-2”\\.`));
    await until('the focus is back on the Rename button', async () => (await activeRename()) === `collection:${c.id}`);
    await page.keyboard.press('Control+z');
    await until('the old name is back', async () => (await collections()).some((x) => x.id === c.id && x.name === brand));
    assert.equal((await snapshot()).seq, seq + 2, 'one undo step');
    await until('the title shows the old name', async () => (await text('[data-testid=manage-title]')) === brand);
  });

  await check('a variable removed by another actor while its field is open closes the field and says it no longer exists', async () => {
    const c = (await collections()).find((x) => x.name === brand);
    const id = `tmp_${run}`;
    await post({ type: 'addVariable', variable: { id, collection: c.id, name: `color/tmp-${run}`, type: 'color', values: Object.fromEntries(c.modes.map((m) => [m.id, '#112233'])) } });
    await $(`[data-testid=manage-var][data-id="${id}"]`).waitFor();
    await renameKey(`variable:${id}`).click();
    await $('[data-testid=manage-renaming-variable]').waitFor();
    await post({ type: 'removeVariable', id });
    await until('the field is gone', async () => (await $('[data-testid=manage-renaming-variable]').count()) === 0);
    assert.match(await text('[data-testid=manage-error]'), /That variable no longer exists\./);
    assert.equal(await dialogOpen(), true);
  });

  await check('an untouched field does not undo a rename by another actor: it says the name changed, and Enter sends nothing and leaves no old status', async () => {
    const c = (await collections()).find((x) => x.name === brand);
    const id = `mv_${run}`;
    await post({ type: 'addVariable', variable: { id, collection: c.id, name: `color/mv-${run}`, type: 'color', values: Object.fromEntries(c.modes.map((m) => [m.id, '#112233'])) } });
    await $(`[data-testid=manage-var][data-id="${id}"]`).waitFor();
    await renameKey(`variable:${id}`).click();
    const f = $('[data-testid=manage-renaming-variable]');
    await f.waitFor();
    assert.equal(await text('[data-testid=manage-renaming-variable-moved]'), '', 'nothing to say yet');
    await post({ type: 'renameVariable', id, name: `color/mv2-${run}` });
    await until('the field says the name changed', async () => (await text('[data-testid=manage-renaming-variable-moved]')) === `This name changed to “color/mv2-${run}” while you were editing.`);
    assert.equal(await f.inputValue(), `color/mv-${run}`, 'the text is left as it was');
    assert.equal(await $('[data-testid=manage-renaming-variable-save]').isDisabled(), false);
    const seq = (await snapshot()).seq;
    await f.press('Enter');
    await until('the field is gone', async () => (await $('[data-testid=manage-renaming-variable]').count()) === 0);
    await sleep(300);
    assert.equal((await snapshot()).seq, seq, 'nothing was sent');
    assert.equal((await variables()).find((v) => v.id === id).name, `color/mv2-${run}`, "the other actor's name stands");
    assert.equal(await text('[data-testid=manage-status]'), '', 'the earlier words are gone');
    assert.equal(await text('[data-testid=manage-error]'), '');
    assert.equal(await activeRename(), `variable:${id}`);
    await post({ type: 'removeVariable', id });
  });

  await check('Detach lets go of the use as a literal and removes the variable; one undo brings both back', async () => {
    const ink = (await variables()).find((v) => v.name === inkName);
    await $(`[data-testid=manage-var][data-id="${ink.id}"] [data-testid=manage-remove-variable]`).click();
    await $('[data-testid=remove-confirm-yes]').click();
    await until('the variable is gone on the server', async () => !(await variables()).some((v) => v.id === ink.id));
    const node = (await snapshot()).nodes.find((x) => x.id === frame);
    assert.equal(typeof node.fill, 'string', 'the field holds a literal now');
    await until('the row is gone', async () => (await $(`[data-testid=manage-var][data-id="${ink.id}"]`).count()) === 0);
    assert.match(await text('[data-testid=manage-status]'), /Detached 1 use and removed “color\/ink-/);
    await until('the focus is back in the view', () => page.evaluate(() => document.querySelector('[data-testid=manage-dialog]')?.contains(document.activeElement)));
    // One step: Ctrl+Z from inside the dialog is the editor's undo.
    await page.keyboard.press('Control+z');
    await until('the variable is back', async () => (await variables()).some((v) => v.id === ink.id));
    await until('the binding is back too', async () => (await snapshot()).nodes.find((x) => x.id === frame)?.fill?.var === ink.id);
    await until('the row is back with its use', async () => (await $(`[data-testid=manage-var][data-id="${ink.id}"] [data-testid=manage-var-uses]`).textContent()) === '1');
  });

  await check('an unused variable and a mode go with a plain removal after a question; the mode keeps the others', async () => {
    const gap = (await variables()).find((v) => v.name === gapName);
    await $(`[data-testid=manage-var][data-id="${gap.id}"] [data-testid=manage-remove-variable]`).click();
    assert.match(await text('[data-testid=remove-confirm]'), /Nothing uses it/);
    assert.equal(await text('[data-testid=remove-confirm-yes]'), 'Remove');
    await $('[data-testid=remove-confirm-yes]').click();
    await until('the number is gone', async () => !(await variables()).some((v) => v.id === gap.id));
    const c = (await collections()).find((x) => x.name === brand);
    await $('[data-testid=manage-remove-mode]').click();
    assert.match(await text('[data-testid=remove-confirm]'), /Remove the mode “dark”\?/);
    await $('[data-testid=remove-confirm-yes]').click();
    await until('one mode left', async () => (await collections()).find((x) => x.id === c.id)?.modes.length === 1);
  });

  await check('removing a collection that holds a used variable asks, says what is used, and detaches', async () => {
    const c = (await collections()).find((x) => x.name === brand);
    await $('[data-testid=manage-remove-collection]').click();
    const q = $('[data-testid=remove-confirm]');
    assert.match(await q.textContent(), /with 1 variable\? There is 1 use in the file \(node fields and aliases/);
    assert.equal(await text('[data-testid=remove-confirm-yes]'), 'Detach');
    await $('[data-testid=remove-confirm-yes]').click();
    await until('the collection is gone on the server', async () => !(await collections()).some((x) => x.id === c.id));
    assert.equal((await variables()).filter((v) => v.collection === c.id).length, 0);
    assert.equal(typeof (await snapshot()).nodes.find((x) => x.id === frame).fill, 'string');
    await until('the list is empty again', async () => (await $('[data-testid=manage-collection]').count()) === 0);
    await shot('after');
  });

  await check('adding a collection while a rename field is open closes the field for good: choosing the first collection again shows no field', async () => {
    await addEntity('manage-new-collection', `fa-${run}`);
    const a = await until('the first collection', async () => (await collections()).find((x) => x.name === `fa-${run}`));
    await until('it is chosen', async () => (await $(`[data-testid=manage-collection][data-id="${a.id}"]`).getAttribute('aria-current')) === 'true');
    await renameKey(`collection:${a.id}`).click();
    await $('[data-testid=manage-renaming-collection]').waitFor();
    await addEntity('manage-new-collection', `fb-${run}`);
    const b = await until('the second collection', async () => (await collections()).find((x) => x.name === `fb-${run}`));
    await until('the second is chosen', async () => (await $(`[data-testid=manage-collection][data-id="${b.id}"]`).getAttribute('aria-current')) === 'true');
    assert.equal(await $('[data-testid=manage-renaming-collection]').count(), 0);
    await $(`[data-testid=manage-collection][data-id="${a.id}"]`).click();
    await until('the first is shown again', async () => (await text('[data-testid=manage-title]')) === `fa-${run}`);
    assert.equal(await $('[data-testid=manage-renaming-collection]').count(), 0, 'the field did not come back');
    assert.equal(await renameKey(`collection:${a.id}`).count(), 1, 'its Rename button is there');
  });

  await check('Escape closes the view and the focus goes back to the Manage button', async () => {
    await page.keyboard.press('Escape');
    await until('the dialog is gone', async () => (await $('[data-testid=manage-dialog]').count()) === 0);
    await until('the focus is on Manage', async () => (await activeTestId()) === 'var-manage');
  });

  await check('no uncaught error in the page', async () => assert.deepEqual(pageErrors, []));
  await browser.close();
  const failed = results.filter((r) => !r).length;
  console.log(failed ? `${failed} of ${results.length} checks failed` : `all ${results.length} checks passed`);
  process.exit(failed ? 1 : 0);
})().catch(async (e) => {
  console.error(`manage-e2e: ${e.message}`);
  await browser?.close().catch(() => {});
  process.exit(1);
});
