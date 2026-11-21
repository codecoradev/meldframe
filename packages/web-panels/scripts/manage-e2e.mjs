// Browser check of the Variables manage view against a real server: node packages/web-panels/scripts/manage-e2e.mjs [web-url] [server-url]
// The manage view in a browser (D8): add a collection, a mode (copying a mode) and variables, the checks of a name before
// anything is sent, the confirmation of a removal with Detach and Cancel, one undo of a detach, and the focus going where
// the person is. Start the server and the page with a file name that is new for every run (the check refuses a file
// that has content), the server on any free port (here 18080) and the token of the page in .dev-token at the root:
//   MELDFRAME_DEV_TOKENS="$(cat .dev-token):ws_dev:agent:dev" MELDFRAME_LISTEN=127.0.0.1:18080 <server binary or just dev-server>
//   MELDFRAME_WEB_FILE=<file> MELDFRAME_WEB_TOKEN="$(cat .dev-token)" MELDFRAME_SERVER=http://127.0.0.1:18080 npx vite --port 15173   (in apps/web)
//   MELDFRAME_WEB_FILE=<file> node packages/web-panels/scripts/manage-e2e.mjs http://localhost:15173 http://127.0.0.1:18080
// The WASM bridge must be built from this revision (`just wasm-build`: the six manage actions are in it) and Playwright
// comes from the global module path. The web and server URLs must be loopback (the token is sent there). It adds a
// frame, collections and variables to the file and leaves them: run it against a development server only.
// MELDFRAME_E2E_SHOTS=<dir> saves screenshots (and one of the page when a check fails).
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';

const { chromium } = createRequire(import.meta.url)('playwright');
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
  await page.goto(web);
  await until('the editor is live', async () => (await page.getAttribute('[data-testid=connection]', 'data-state')) === 'live');
  await until('the frame is listed', async () => (await $(`[data-testid=layer][data-id="${frame}"]`).count()) === 1);

  await check('the Manage button opens the view, with the focus inside it and the scrim over the page', async () => {
    await $('[data-testid=var-manage]').click();
    await $('[data-testid=manage-dialog]').waitFor();
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
