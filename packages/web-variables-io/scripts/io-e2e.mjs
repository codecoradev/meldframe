// Browser check of the Export variables and Import variables dialogs against a real server:
//   node packages/web-variables-io/scripts/io-e2e.mjs <web-url> <server-url>
// Export (D8): the dialog lists every variable of the file as CSS, one declaration per line, with the :root block and a rule
// for each other mode; it follows a change from outside; Copy puts the text on the clipboard and Download saves variables.css;
// Escape closes it and the focus goes back to the button. Import: text pasted into the field is imported as one undo step with
// its report, a second import of the same names is refused naming the first and sends nothing, text with no variable says
// so, a text of 51 variables is refused with the pointer to the agent tool import_tokens and nothing is sent (the dialog
// takes at most 50), one undo takes the whole import back and the open dialog stops saying "Imported", closing the dialog
// keeps the text until Clear, a file chosen with the file chooser fills the field and imports, and a file that is too large is
// refused (by the page before it is read, or by the app layer with both sizes) without sending anything.
// Start the server and the page the way the header of packages/web-variables/scripts/manage-e2e.mjs says (the token of the
// page is the content of the git-ignored .dev-token at the root, handed to the server and to the page's proxy through the
// environment named there, never on a command line). Both URLs are required arguments and have no default, they must be loopback
// (127.0.0.1, localhost or ::1: the token is sent there), and the environment variable MELDFRAME_WEB_FILE must name a file that is new
// for this run: the check refuses a file that already has content, so it cannot write into a file someone works in. The WASM
// bridge must be built from this revision (`just wasm-build`: the Export variables and Import variables actions are in it) and
// Playwright comes from the global module path. It adds collections and variables to the file and leaves them.
// The checks run in order and share the file: a check that fails can fail the ones after it.
// MELDFRAME_E2E_SHOTS=<dir> saves screenshots (and one of the page when a check fails).
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';

const { chromium } = createRequire(import.meta.url)('playwright');
const [web, server] = [process.argv[2], process.argv[3]];
if (!web || !server) throw new Error('usage: node io-e2e.mjs <web-url> <server-url> (both loopback; see the header of this file)');
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
    body: JSON.stringify({ ops: [{ key: `io-${run}-${++n}`, command, deps: [] }] }),
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
const inside = (sel) => page.evaluate((s) => document.querySelector(s)?.contains(document.activeElement) ?? false, sel);
const results = [];
async function check(name, fn) {
  try {
    await fn();
    results.push(true);
    console.log(`ok   ${name}`);
  } catch (e) {
    results.push(false);
    if (shots && page) await page.screenshot({ path: `${shots}/io-fail-${results.length}.png` }).catch(() => {});
    console.log(`FAIL ${name}: ${String(e.message ?? e).split('\n').slice(0, process.env.MELDFRAME_E2E_DEBUG ? 12 : 1).join(' | ')}`);
  }
}
const shot = (name) => (shots && page ? page.screenshot({ path: `${shots}/io-${name}.png` }) : null);

const THEME = `:root {
  --background: #ffffff;
  --foreground: #0a0a0a;
  --primary: var(--foreground);
  --radius: 8px;
  --shadow: 0 1px 2px rgb(0 0 0 / 0.1);
}
.dark {
  --background: #0a0a0a;
  --foreground: #fafafa;
}
@theme {
  --color-brand-500: oklch(0.6 0.2 250);
  --spacing-4: 1rem;
}
`;
const frame = `frame_${run}`;
const openExport = async () => {
  await $('[data-testid=var-export]').click();
  await $('[data-testid=export-vars-dialog]').waitFor();
};
const openImport = async () => {
  await $('[data-testid=var-import]').click();
  await $('[data-testid=import-vars-dialog]').waitFor();
};
// The sequence number once the server has stopped taking ops (an undo sends a few, one after the other).
const quiet = async () => {
  let seq = (await snapshot()).seq;
  for (;;) {
    await sleep(400);
    const next = (await snapshot()).seq;
    if (next === seq) return seq;
    seq = next;
  }
};
const gone = (testid) => until(`${testid} is gone`, async () => (await $(`[data-testid=${testid}]`).count()) === 0);
const codeLines = async () => (await $('[data-testid=export-vars-code] > div').allTextContents()).map((l) => l.replace(/^\d+/, ''));

(async () => {
  assert.equal((await snapshot()).seq, 0, 'the file has content already: start the page and the check with a file name that is new');
  await post({ type: 'addNode', node: { id: frame, name: `Card ${run}`, type: 'frame', parentId: null, x: 0, y: 0, w: 300, h: 200, fill: '#ffffff' } });
  browser = await chromium.launch({ args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
  const context = await browser.newContext({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1, acceptDownloads: true });
  await context.grantPermissions(['clipboard-read', 'clipboard-write'], { origin: new URL(web).origin });
  page = await context.newPage();
  page.on('pageerror', (e) => { pageErrors.push(e.message); console.log('[pageerror]', e.message); });
  await page.goto(web);
  await until('the editor is live', async () => (await page.getAttribute('[data-testid=connection]', 'data-state')) === 'live');
  await until('the frame is listed', async () => (await $(`[data-testid=layer][data-id="${frame}"]`).count()) === 1);

  await check('the header of the Variables section has the two buttons, named Import variables and Export variables', async () => {
    assert.equal((await text('[data-testid=var-import]')), 'Import variables');
    assert.equal((await text('[data-testid=var-export]')), 'Export variables');
  });

  await check('Export on a file with no variables says so, has nothing to copy, sends nothing; Escape closes it and the focus goes back to the button', async () => {
    const seq = (await snapshot()).seq;
    await openExport();
    await until('the focus is in the dialog', () => inside('[data-testid=export-vars-dialog]'));
    assert.match(await text('[data-testid=export-vars-empty]'), /no variables yet/i);
    assert.equal(await $('[data-testid=export-vars-copy]').isDisabled(), true);
    assert.equal(await $('[data-testid=export-vars-download]').isDisabled(), true);
    await shot('export-empty');
    await page.keyboard.press('Escape');
    await gone('export-vars-dialog');
    await until('the focus is back on Export variables', async () => (await activeTestId()) === 'var-export');
    await sleep(300);
    assert.equal((await snapshot()).seq, seq, 'opening and closing sent nothing');
  });

  // A file with two collections, a mode, an alias and a variable no frame uses.
  const brand = { id: `c-app-${run}`, name: 'appearance', modes: [{ id: `m-l-${run}`, name: 'Light' }, { id: `m-d-${run}`, name: 'Dark' }] };
  const sizes = { id: `c-size-${run}`, name: 'sizes', modes: [{ id: `m-s-${run}`, name: 'Default' }] };
  const vBrand = `v-brand-${run}`;
  await post({ type: 'addCollection', collection: brand });
  await post({ type: 'addCollection', collection: sizes });
  await post({ type: 'addVariable', variable: { id: vBrand, collection: brand.id, name: 'color/brand', type: 'color', values: { [brand.modes[0].id]: '#112233', [brand.modes[1].id]: '#AABBCC' } } });
  await post({ type: 'addVariable', variable: { id: `v-acc-${run}`, collection: brand.id, name: 'color/accent', type: 'color', values: { [brand.modes[0].id]: { var: vBrand }, [brand.modes[1].id]: '#000000' } } });
  await post({ type: 'addVariable', variable: { id: `v-rad-${run}`, collection: sizes.id, name: 'radius/card', type: 'number', values: { [sizes.modes[0].id]: 12 } } });
  await until('the page has the variables', async () => /of 3$/.test(await text('[data-testid=var-count]')));

  await check('Export lists every variable as CSS, one declaration per line: the :root block, a rule for the other mode, an alias as var(), the colour as its stored hex, lowercased', async () => {
    await openExport();
    assert.match(await text('[data-testid=export-vars-summary]'), /^3 variables in 2 collections, 2 rules, /);
    const lines = await codeLines();
    assert.deepEqual(lines.slice(0, 7), [
      ':root {',
      '  --appearance-color-brand: #112233;',
      '  --appearance-color-accent: var(--appearance-color-brand);',
      '  --sizes-radius-card: 12;',
      '}',
      ':root[data-mode-appearance="dark"] {',
      '  --appearance-color-brand: #aabbcc;',
    ]);
    assert.equal(lines.at(-1), '}');
    assert.match(await text('[data-testid=export-vars-help]'), /CSS custom properties/);
    assert.match(await page.locator('[data-testid=export-vars-help]').nth(1).textContent(), /collection in front/);
    await until('the focus is in the dialog', () => inside('[data-testid=export-vars-dialog]'));
    await shot('export');
  });

  await check('Copy puts the whole text on the clipboard and says so; Download saves it as variables.css', async () => {
    await $('[data-testid=export-vars-copy]').click();
    await until('copied', async () => (await text('[data-testid=export-vars-said]')) === 'Copied.');
    const clip = await page.evaluate(() => navigator.clipboard.readText());
    assert.equal(clip, (await codeLines()).join('\n') + '\n');
    assert.ok(clip.includes('--sizes-radius-card: 12;'));
    const [download] = await Promise.all([page.waitForEvent('download'), $('[data-testid=export-vars-download]').click()]);
    assert.equal(download.suggestedFilename(), 'variables.css');
    assert.equal(readFileSync(await download.path(), 'utf8'), clip);
  });

  await check('the open dialog follows a change from outside, and builds from the same file', async () => {
    await post({ type: 'addVariable', variable: { id: `v-op-${run}`, collection: sizes.id, name: 'opacity/muted', type: 'number', values: { [sizes.modes[0].id]: 0.4 } } });
    await until('the text has the new variable', async () => (await codeLines()).some((l) => l.includes('--sizes-opacity-muted: 0.4;')));
    assert.match(await text('[data-testid=export-vars-summary]'), /^4 variables in 2 collections/);
  });

  await check('Escape closes the dialog and the focus goes back to Export variables; the file was not changed by the dialog', async () => {
    const seq = (await snapshot()).seq;
    await page.keyboard.press('Escape');
    await gone('export-vars-dialog');
    await until('the focus is back on Export variables', async () => (await activeTestId()) === 'var-export');
    assert.equal((await snapshot()).seq, seq);
  });

  const before = { vars: (await variables()).length, cols: (await collections()).length };

  await check('Import variables opens with the focus in the field and Import off while it is empty', async () => {
    await openImport();
    await until('the focus is in the field', async () => (await activeTestId()) === 'import-vars-text');
    assert.equal(await $('[data-testid=import-vars-import]').isDisabled(), true);
    assert.match(await text('[data-testid=import-vars-help]'), /Nothing in the text is run or fetched/);
    assert.match(await $('[data-testid=import-vars-help]').nth(1).textContent(), /At most 50 variables at a time/);
    assert.match(await $('[data-testid=import-vars-help]').nth(2).textContent(), /only adds/);
    assert.equal(await page.getByLabel('CSS to import').getAttribute('data-testid'), 'import-vars-text', 'the field has a visible label');
    await shot('import-empty');
  });

  await check('pasted text is imported as one step: the result says what was added, the report lists what was skipped, the field is emptied', async () => {
    const seq = (await snapshot()).seq;
    await $('[data-testid=import-vars-text]').fill(THEME);
    assert.equal(await $('[data-testid=import-vars-import]').isDisabled(), false);
    await $('[data-testid=import-vars-import]').click();
    await $('[data-testid=import-vars-result][data-status=imported]').waitFor();
    assert.equal(await text('[data-testid=import-vars-title]'), 'Imported');
    assert.match(await text('[data-testid=import-vars-line]'), /^2 collections, 3 modes and 6 variables were added: \d+ colours? and \d+ numbers?, 1 alias among them\.$/);
    assert.match(await $('[data-testid=import-vars-line]').nth(1).textContent(), /1 custom property was left out/);
    assert.match(await text('[data-testid=import-vars-report]'), /--shadow/);
    assert.equal(await $('[data-testid=import-vars-text]').inputValue(), '');
    // Import and Clear are off now: the focus must not stay on one of them, inside a modal, with nothing to act on.
    await until('the focus is on a control that works', async () => page.evaluate(() => {
      const a = document.activeElement;
      return !!a && a !== document.body && !a.disabled && !!document.querySelector('[data-testid=import-vars-dialog]')?.contains(a);
    }));
    assert.equal(await activeTestId(), 'import-vars-done');
    const added = await until('the import on the server', async () => ((await variables()).length === before.vars + 6 ? await collections() : null));
    assert.ok(added.some((c) => c.id === 'col-root') && added.some((c) => c.id === 'col-theme'));
    assert.equal(added.length, before.cols + 2);
    assert.ok((await snapshot()).seq >= seq + 8, 'one op for each command');
    await shot('import-done');
  });

  await check('a second import of the same names is refused, names the first, and sends nothing', async () => {
    const seq = (await snapshot()).seq;
    await $('[data-testid=import-vars-text]').fill(THEME);
    assert.equal(await $('[data-testid=import-vars-result]').getAttribute('data-status'), '', 'editing the text puts the old result away');
    await $('[data-testid=import-vars-import]').click();
    await $('[data-testid=import-vars-result][data-status=refused]').waitFor();
    assert.equal(await text('[data-testid=import-vars-title]'), 'Nothing was imported');
    assert.match(await text('[data-testid=import-vars-line]'), /^The collection root is already in this file\. An import only adds/);
    assert.equal(await $('[data-testid=import-vars-text]').inputValue(), THEME, 'the text stays for the person to change');
    await sleep(400);
    assert.equal((await snapshot()).seq, seq, 'nothing was sent');
    await shot('import-refused');
  });

  await check('text with no variable says so and sends nothing; empty text cannot be sent', async () => {
    const seq = (await snapshot()).seq;
    await $('[data-testid=import-vars-text]').fill('.card { --local: #fff; } body { color: red; }');
    await $('[data-testid=import-vars-import]').click();
    await $('[data-testid=import-vars-result][data-status=nothing]').waitFor();
    assert.equal(await text('[data-testid=import-vars-title]'), 'Nothing to import');
    assert.ok((await $('[data-testid=import-vars-report] li').count()) > 0, 'the report says what was skipped');
    await $('[data-testid=import-vars-text]').fill('   ');
    assert.equal(await $('[data-testid=import-vars-import]').isDisabled(), true);
    await sleep(300);
    assert.equal((await snapshot()).seq, seq);
  });

  await check('the built CSS styles the dialog: the field resizes and has a least height, the report list scrolls, the badges have a width, and a focused field changes its border', async () => {
    await $('[data-testid=import-vars-text]').fill('.card { --local: #fff; } body { color: red; }');
    await $('[data-testid=import-vars-import]').click();
    await $('[data-testid=import-vars-result][data-status=nothing]').waitFor();
    await page.mouse.move(0, 0);
    await $('[data-testid=import-vars-choose]').focus();
    const unfocused = await $('[data-testid=import-vars-text]').evaluate((el) => getComputedStyle(el).borderTopColor);
    await $('[data-testid=import-vars-text]').focus();
    const s = await page.evaluate(() => {
      const probe = document.createElement('div');
      probe.style.width = 'var(--mf-space-1)';
      document.body.append(probe);
      const unit = Number.parseFloat(getComputedStyle(probe).width);
      probe.remove();
      const css = (sel) => getComputedStyle(document.querySelector(sel));
      const field = css('[data-testid=import-vars-text]');
      return {
        unit,
        resize: field.resize,
        minHeight: field.minHeight,
        focused: field.borderTopColor,
        listMaxHeight: css('[data-testid=import-vars-report] ul').maxHeight,
        badgeWidth: css('[data-testid=import-vars-report] li span').width,
        sectionTop: css('[data-testid=import-vars-report]').marginTop,
      };
    });
    assert.ok(s.unit > 0, 'the spacing unit is a length');
    assert.equal(s.resize, 'vertical', 'the field can be resized in height');
    assert.equal(s.minHeight, `${40 * s.unit}px`, 'the field has a least height');
    assert.equal(s.listMaxHeight, `${48 * s.unit}px`, 'the report list has a greatest height and scrolls');
    assert.equal(s.badgeWidth, `${20 * s.unit}px`, 'the kind badge of a report line has its width');
    assert.equal(s.sectionTop, `${3 * s.unit}px`, 'the report sits apart from the result');
    assert.notEqual(s.focused, unfocused, 'a focused field draws its border in the selection colour');
  });

  await check('Escape closes the dialog and the focus goes back to Import variables; one undo takes the whole import back', async () => {
    await page.keyboard.press('Escape');
    await gone('import-vars-dialog');
    await until('the focus is back on Import variables', async () => (await activeTestId()) === 'var-import');
    await page.keyboard.press('Control+z');
    await until('the import is gone from the server', async () => (await variables()).length === before.vars && (await collections()).length === before.cols);
    assert.equal((await variables()).length, before.vars);
  });

  await check('closing the dialog keeps the text in the field until Clear; Clear empties it and the note', async () => {
    await openImport();
    await $('[data-testid=import-vars-text]').fill('keep me');
    await page.keyboard.press('Escape');
    await gone('import-vars-dialog');
    await openImport();
    assert.equal(await $('[data-testid=import-vars-text]').inputValue(), 'keep me', 'Escape is not a Clear');
    await $('[data-testid=import-vars-clear]').click();
    assert.equal(await $('[data-testid=import-vars-text]').inputValue(), '');
    assert.equal(await $('[data-testid=import-vars-clear]').isDisabled(), true);
    await page.keyboard.press('Escape');
    await gone('import-vars-dialog');
  });

  await check('a file chosen with the file chooser fills the field, says it was read, and imports', async () => {
    await openImport();
    assert.equal(await $('[data-testid=import-vars-text]').inputValue(), '', 'a dialog opened again starts with an empty field');
    assert.equal(await $('[data-testid=import-vars-result]').getAttribute('data-status'), '', 'and no result');
    const [chooser] = await Promise.all([page.waitForEvent('filechooser'), $('[data-testid=import-vars-choose]').click()]);
    await chooser.setFiles({ name: 'theme.css', mimeType: 'text/css', buffer: Buffer.from(THEME) });
    await until('the field has the text', async () => (await $('[data-testid=import-vars-text]').inputValue()) === THEME);
    assert.match(await text('[data-testid=import-vars-note]'), /^Read theme\.css \(\d+ bytes\)\./);
    await $('[data-testid=import-vars-import]').click();
    await $('[data-testid=import-vars-result][data-status=imported]').waitFor();
    await until('the import on the server', async () => (await variables()).length === before.vars + 6);
    assert.equal(await $('[data-testid=import-vars-text]').inputValue(), '', 'an applied import empties the field');
    assert.equal(await text('[data-testid=import-vars-note]'), '', 'and the note about the file');
  });

  await check('an undo while the dialog is open takes the import back and the dialog stops saying Imported; the import can be made again', async () => {
    await page.keyboard.press('Control+z');
    await until('the import is gone from the server', async () => (await variables()).length === before.vars);
    await until('the result is gone', async () => (await $('[data-testid=import-vars-result]').getAttribute('data-status')) === '');
    assert.equal(await $('[data-testid=import-vars-title]').count(), 0);
  });

  await check('a text of 51 variables is refused with the pointer to import_tokens, its count and the cap, and nothing is sent', async () => {
    const seq = await quiet();
    const many = `:root {${Array.from({ length: 51 }, (_, i) => ` --many-${i}: ${i}px;`).join('')} }`;
    await $('[data-testid=import-vars-text]').fill(many);
    await $('[data-testid=import-vars-import]').click();
    await $('[data-testid=import-vars-result][data-status=refused]').waitFor();
    assert.equal(await text('[data-testid=import-vars-title]'), 'Nothing was imported');
    const line = await text('[data-testid=import-vars-line]');
    assert.match(line, /^This text has more than 50 variables and the dialog imports at most 50 at a time/);
    assert.match(line, /import_tokens/);
    assert.equal(await $('[data-testid=import-vars-text]').inputValue(), many, 'the text stays');
    await sleep(500);
    assert.equal((await snapshot()).seq, seq, 'nothing was sent');
    assert.equal((await variables()).length, before.vars);
    // 50 is within the cap: the same text without its last variable imports, and the server confirms all of its ops.
    await $('[data-testid=import-vars-text]').fill(`:root {${Array.from({ length: 50 }, (_, i) => ` --many-${i}: ${i}px;`).join('')} }`);
    await $('[data-testid=import-vars-import]').click();
    await $('[data-testid=import-vars-result][data-status=imported]').waitFor();
    await until('the 50 variables are confirmed by the server', async () => (await variables()).length === before.vars + 50);
    assert.match(await text('[data-testid=import-vars-line]'), /^1 collection, 1 mode and 50 variables were added/);
    await shot('import-50');
    // One undo takes the 50 back (the dialog is open, the focus is on a button).
    await page.keyboard.press('Control+z');
    await until('the 50 are gone from the server', async () => (await variables()).length === before.vars, 60000);
  });

  await check('the theme is imported again for the export that follows', async () => {
    await $('[data-testid=import-vars-text]').fill(THEME);
    await $('[data-testid=import-vars-import]').click();
    await $('[data-testid=import-vars-result][data-status=imported]').waitFor();
    await until('the import on the server again', async () => (await variables()).length === before.vars + 6);
  });

  await check('a file over the limit of the import is refused by the app layer with both sizes, and nothing is sent; one over 1 MiB is not read', async () => {
    const seq = (await snapshot()).seq;
    const big = `:root { --a: #fff; }${' '.repeat(300 * 1024)}`;
    const [c1] = await Promise.all([page.waitForEvent('filechooser'), $('[data-testid=import-vars-choose]').click()]);
    await c1.setFiles({ name: 'big.css', mimeType: 'text/css', buffer: Buffer.from(big) });
    await until('the field has the text', async () => (await $('[data-testid=import-vars-text]').inputValue()).length === big.length);
    await $('[data-testid=import-vars-import]').click();
    await $('[data-testid=import-vars-result][data-status=refused]').waitFor();
    assert.match(await text('[data-testid=import-vars-line]'), /300\.0 KiB and an import reads at most 256\.0 KiB/);
    const [c2] = await Promise.all([page.waitForEvent('filechooser'), $('[data-testid=import-vars-choose]').click()]);
    await c2.setFiles({ name: 'huge.css', mimeType: 'text/css', buffer: Buffer.alloc(1024 * 1024 + 1, 32) });
    await until('the note says it was not read', async () => /not read here/.test(await text('[data-testid=import-vars-note]')));
    assert.equal((await $('[data-testid=import-vars-text]').inputValue()).length, big.length, 'the field is as it was');
    await sleep(300);
    assert.equal((await snapshot()).seq, seq, 'nothing was sent');
  });

  await check('Export after the import lists the imported variables under their names, with the dark mode rule', async () => {
    await page.keyboard.press('Escape');
    await gone('import-vars-dialog');
    await openExport();
    const lines = await codeLines();
    assert.ok(lines.includes('  --root-background: #ffffff;'), lines.join('|'));
    assert.ok(lines.includes(':root[data-mode-root="dark"] {'));
    assert.ok(lines.includes('  --root-primary: var(--root-foreground);'));
    assert.ok(lines.includes('  --theme-spacing-4: 16;'));
    await page.keyboard.press('Escape');
    await gone('export-vars-dialog');
  });

  await browser.close();
  assert.deepEqual(pageErrors, [], 'no page errors');
  const failed = results.filter((r) => !r).length;
  console.log(failed ? `${failed} of ${results.length} checks failed` : `all ${results.length} checks passed`);
  process.exit(failed ? 1 : 0);
})().catch(async (e) => {
  console.error(e);
  if (shots && page) await page.screenshot({ path: `${shots}/io-error.png` }).catch(() => {});
  await browser?.close().catch(() => {});
  process.exit(1);
});
