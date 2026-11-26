// Pixel comparison of the Svelte and the React copy of a panel: node packages/web-panels/scripts/pixels-e2e.mjs [web-url] [server-url]
// Needs the development server (the server binary with MELDFRAME_DEV_TOKENS and no DATABASE_URL, token in .dev-token at the root) and
// Vite running, the WASM bridge built, and Playwright from the global module path; MELDFRAME_WEB_FILE names a file that is new for this run
// (the script adds a frame, four nodes, a collection and three variables to it, and refuses a file that has content already: development server only). Hand-run, not in CI.
// A second token among the server's development tokens, with another actor (MELDFRAME_E2E_TOKEN2, as editor-e2e.mjs has it), makes the last
// node come from another actor, so that the Layers panel's marker of who changed a node is imaged; without it that is not imaged and the
// run says SKIP. The image picker of the Fill section is imaged in two places. In the Properties panel it is imaged open on a workspace that
// holds no image (a server that has just started, or one that no image was uploaded to; image-fill-e2e.mjs uploads some): the step waits for the
// note of an empty workspace and fails without it. The panel `images` (the word `properties`, the element the picker itself) uploads six
// images to the workspace of the token before it opens either copy, so it comes after the empty workspace and a second run needs a server that
// has just started (it adds a rectangle with an image fill, `im1`): six tiles with the first focused, the cursor moved by the keyboard, the tile of
// the image the node has marked, a preview that did not load, a failed list, Load more and Load more while the next page comes. The failed list
// and the next page are made with the page's routes (a 503, a list that has a cursor, a request that is held), the same on both copies.
// For each panel it opens the editor twice on the same file, once with the Svelte copy and once with the React copy (`/?react=<panel>`), puts
// the page through the same steps, takes an image of the panel's element at 1440x900 (1440x1800 for the Properties panel, which is taller than the card) and a device scale factor of 2 after each step, and
// compares the two images pixel by pixel. The tolerance is 0: one differing pixel fails the run. Before it compares, each React page is
// asked that React drew the element and the Svelte page that it did not, so a silent fallback to Svelte cannot pass. A panel is a row of
// PANELS: the switch word, the element, and the steps (each step ends in a state to compare).
// Not covered: the other themes, other widths, a hover or focus ring that the steps do not leave on screen, text of other languages, the
// Layers panel's empty state (the file has nodes), the icon of a shader node and, in the Properties panel, the shader node, the Text
// section's weight select open, the chip of a variable that was removed (a binding whose variable is gone) and Absolute position
// checked. A step only clicks and types: it never changes the file, so the second run starts from the file
// the first one saw; the one exception is the import steps of `io-import`, which apply a style sheet and whose `finish` takes it back with one undo (and waits
// for the server to have it) before the React page opens.
// The Activity panel is imaged by pixels-activity-e2e.mjs: its rows are live (a batch the page watched arrive can be reverted, one that was
// there when the page loaded cannot) and its lanes change with the clock, so it opens both copies at the same time on one file.
// Not imaged in the manage view (`manage`, the dialog's element, opened by its Manage button): the question before removing a mode, the partial-counts notice, the error and status lines and a file with no collection.
// The Export variables and Import variables dialogs are `io-export` and `io-import` (word `io`, opened by the two buttons of the Variables section's header). Not imaged
// there: the export of a file with no variable, of a text over the limit, of a long preview cut at 400 lines and with warnings (the file has none of those), the import
// of a text over 256 KiB, a report cut at its cap, a refusal naming a mode or a variable, and a file that could not be read.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { pngOf, uploadPng } from './png-helpers.mjs';
import { fibre, makeDiffer } from './pixels-helpers.mjs';

const { chromium } = createRequire(import.meta.url)('playwright');
const web = process.argv[2] ?? 'http://localhost:5173';
const server = process.argv[3] ?? 'http://127.0.0.1:8080';
const root = new URL('../../../', import.meta.url).pathname;
const token = readFileSync(`${root}.dev-token`, 'utf8').trim();
const file = process.env.MELDFRAME_WEB_FILE;
if (!file) throw new Error('set MELDFRAME_WEB_FILE to a file name that is new for this run (see the top of this file)');
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let n = 0;

const token2 = process.env.MELDFRAME_E2E_TOKEN2;
async function post(command, as = token) {
  const res = await fetch(`${server}/v1/files/${file}/tools/apply_commands`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', authorization: `Bearer ${as}` },
    body: JSON.stringify({ ops: [{ key: `pixels-e2e-${++n}`, command, deps: [] }] }),
  });
  assert.equal(res.status, 200, `op refused: ${res.status} ${await res.text()}`);
}
const layer = (page, id) => page.locator(`[data-testid=layer][data-id=${id}]`);
/** Selects a node by its row of the Layers panel and waits for the Properties panel to show it. */
const pick = async (page, id) => {
  const name = (await layer(page, id).locator('button').last().innerText()).trim();
  await layer(page, id).locator('button').last().click();
  await page.waitForFunction((n) => document.querySelector('[data-testid=prop-name]')?.value === n, name);
};

const imageRadio = (page) => page.locator('[data-testid=prop-fill-type]').getByRole('radio', { name: 'Image', exact: true });
/** Waits until the tiles of the picker are there and their thumbnails have loaded (a tile that says No preview has no image to wait for). */
const tilesLoaded = (page, n = 6) =>
  page.waitForFunction((n) => { const t = [...document.querySelectorAll('[data-testid=image-tile]')]; return t.length === n && t.every((x) => { const i = x.querySelector('img'); return i ? i.complete && i.naturalWidth > 0 : true; }); }, n, { timeout: 20000 });
/** Closes the picker with its button when it is open, then opens it again from the type switch of the selected node. */
const reopen = async (page) => {
  await sleep(1200); // the server's budget of calls refills at a rate: a picker opened at once after another asks for six more thumbnails
  if (await page.locator('[data-testid=image-picker]').count()) await page.locator('[data-testid=image-picker-close]').click();
  await imageRadio(page).click();
  await page.locator('[data-testid=image-picker]').waitFor();
};
const routes = new WeakMap();
/** A route of the page that the next call of `clearRoutes` takes away. */
const routeOnce = async (page, match, handler) => { await page.route(match, handler); routes.set(page, [...(routes.get(page) ?? []), [match, handler]]); };
const clearRoutes = async (page) => { for (const [m, h] of routes.get(page) ?? []) await page.unroute(m, h); routes.delete(page); };
const LIST = /\/v1\/assets\?/;
const PALETTE = ['#e91e63', '#00bcd4', '#8bc34a', '#ff9800', '#673ab7', '#009688'];
let imageIds = [];

/** The panels this script compares: the word of `/?react=`, the element to image, the steps. Each step leaves the page in a state to compare. */
const PANELS = {
  layers: {
    word: 'layers',
    element: '[data-testid=layers]',
    steps: [
      { name: 'the tree', run: async () => {} },
      { name: 'a selected row', run: (p) => layer(p, 'r1').locator('button').last().click() },
      { name: 'a second row added with Shift', run: (p) => layer(p, 'e1').locator('button').last().click({ modifiers: ['Shift'] }) },
      { name: 'the parent folded', run: (p) => layer(p, 'f1').locator('button[aria-expanded]').click() },
    ],
  },
  variables: {
    word: 'variables',
    element: '[data-testid=variables]',
    steps: [
      { name: 'the list', run: async () => {} },
      { name: 'a node selected (the bind buttons)', run: (p) => layer(p, 'f1').locator('button').last().click() },
      { name: 'a value field focused', run: (p) => p.locator('[data-testid=var-row][data-id=v1] [data-testid=var-value]').focus() },
      { name: 'a search that narrows the list', run: (p) => p.fill('[data-testid=var-search]', 'ink') },
    ],
  },
  properties: {
    word: 'properties',
    element: '[data-testid=properties]',
    viewport: { width: 1440, height: 1800 },
    steps: [
      { name: 'nothing selected', run: async () => {} },
      { name: 'a frame', run: (p) => pick(p, 'f1') },
      { name: 'a rectangle', run: (p) => pick(p, 'r1') },
      { name: 'a text', run: (p) => pick(p, 't1') },
      { name: 'a frame with a row layout, padding, a border and a shadow', run: (p) => pick(p, 'f2') },
      { name: 'a rectangle in an auto layout parent (the margin section)', run: (p) => pick(p, 'r2') },
      { name: 'a linear gradient fill', run: (p) => pick(p, 'g1') },
      { name: 'a fill, an opacity and a corner radius bound to variables', run: (p) => pick(p, 'b1') },
      { name: 'a radial gradient', run: (p) => pick(p, 'g2') },
      { name: 'padding that differs by side (the Mixed placeholder) and four shadows (Add shadow off)', run: (p) => pick(p, 'f3') },
      { name: 'a field focused', run: (p) => p.locator('[data-testid=prop-x]').focus() },
      {
        name: 'the image picker open (a workspace with no image)',
        run: async (p) => {
          await pick(p, 'r1');
          await p.locator('[data-testid=prop-fill-type]').getByRole('radio', { name: 'Image', exact: true }).click();
          // The note of an empty workspace: the picker has read its list (a workspace with images would be imaged before its thumbnails load).
          await p.locator('[data-testid=image-picker-empty]').waitFor({ timeout: 20000 });
        },
      },
    ],
  },
  picker: {
    word: 'variables',
    element: '[data-testid=picker]',
    steps: [
      {
        name: 'open on Fill',
        run: async (p) => {
          await layer(p, 'f1').locator('button').last().click();
          await p.locator('[data-testid=prop-fill-bind]').click();
          await p.locator('[data-testid=picker-row]').first().waitFor();
        },
      },
      { name: 'a search', run: (p) => p.fill('[data-testid=picker-search]', 'ink') },
      { name: 'a search with no match', run: (p) => p.fill('[data-testid=picker-search]', 'zzz') },
    ],
  },
  images: {
    word: 'properties',
    element: '[data-testid=image-picker]',
    viewport: { width: 1440, height: 1800 },
    // Six images that differ in their stripes, and a rectangle that has one of them as its fill; the workspace holds no other image.
    before: async () => {
      imageIds = [];
      for (let i = 0; i < PALETTE.length; i++) imageIds.push(await uploadPng(server, token, pngOf(24, 24, (y) => PALETTE[(i + Math.floor(y / 8)) % PALETTE.length])));
      imageIds.sort();
      await post({ type: 'addNode', node: { id: 'im1', name: 'Pictured', type: 'rect', parentId: 'f1', x: 300, y: 220, w: 80, h: 60, fill: '#335577', image: { assetId: imageIds[4], fit: 'contain' } } });
    },
    steps: [
      {
        name: 'open on a rectangle: six tiles, the first focused',
        run: async (p) => {
          await pick(p, 'r1');
          await sleep(1200);
          await imageRadio(p).click();
          await tilesLoaded(p);
        },
      },
      { name: 'the cursor a row down, moved by the keyboard', run: (p) => p.keyboard.press('ArrowDown') },
      {
        name: 'open on a node that has an image: its tile marked as current',
        run: async (p) => {
          await pick(p, 'im1');
          await sleep(1200);
          await p.locator('[data-testid=prop-fill-image-change]').click();
          await tilesLoaded(p);
        },
      },
      {
        name: 'a preview that did not load: No preview, the note and Retry previews',
        run: async (p) => {
          await pick(p, 'r1');
          await routeOnce(p, new RegExp(`/v1/assets/${imageIds[1]}$`), (r) => r.abort());
          await sleep(1200);
          await imageRadio(p).click();
          await p.locator('[data-testid=image-picker-broken]').waitFor({ timeout: 20000 });
          await tilesLoaded(p);
        },
      },
      {
        name: 'a failed list: the alert and Try again, no tile',
        run: async (p) => {
          await clearRoutes(p);
          await routeOnce(p, LIST, (r) => r.fulfill({ status: 503, contentType: 'application/json', body: JSON.stringify({ error: { code: 'unavailable', message: 'busy', retryable: true } }) }));
          await reopen(p);
          await p.locator('[data-testid=image-picker-error]').waitFor({ timeout: 20000 });
        },
      },
      {
        name: 'Load more: a list that has a cursor',
        run: async (p) => {
          await clearRoutes(p);
          await routeOnce(p, LIST, async (r) => {
            const res = await r.fetch();
            const body = await res.json();
            await r.fulfill({ response: res, json: { ...body, next: body.assets.at(-1).id } });
          });
          await reopen(p);
          await p.locator('[data-testid=image-picker-more]').waitFor({ timeout: 20000 });
          await tilesLoaded(p);
        },
      },
      {
        name: 'Load more while the next page comes: the request is held',
        run: async (p) => {
          await clearRoutes(p);
          // The next page is asked for and never answered; the page's context ends with the run.
          await routeOnce(p, LIST, () => new Promise(() => {}));
          await p.locator('[data-testid=image-picker-more]').click();
          await p.getByText('Loading…', { exact: true }).waitFor({ timeout: 20000 });
        },
      },
    ],
  },
};

PANELS.manage = {
  word: 'manage',
  element: '[data-testid=manage-dialog]',
  steps: [
    { name: 'open on the first collection', run: async (p) => { await p.click('[data-testid=var-manage]'); await p.locator('[data-testid=manage-var]').first().waitFor(); } },
    { name: 'a name refused in the new collection field', run: (p) => p.fill('[data-testid=manage-new-collection]', 'Colour') },
    { name: 'a mode name that exists already (a warning)', run: (p) => p.fill('[data-testid=manage-new-mode]', 'dark') },
    {
      name: 'the rename field of a variable, with a name refused',
      run: async (p) => {
        await p.click('[data-rename="variable:v2"]');
        await p.fill('[data-testid=manage-renaming-variable]', 'Color/Ink');
      },
    },
    {
      name: 'the rename field of the first mode',
      run: async (p) => {
        await p.keyboard.press('Escape');
        await p.click('[data-rename="mode:light"]');
      },
    },
    {
      name: 'the question before removing a variable',
      run: async (p) => {
        await p.keyboard.press('Escape');
        await p.click('[data-remove="variable:v3"]');
        await p.locator('[data-testid=remove-confirm]').waitFor();
      },
    },
    { name: 'the question before removing a collection', run: (p) => p.click('[data-testid=manage-remove-collection]') },
  ],
};

// The Export variables and Import variables dialogs (S10, the word `io`). Both are opened by the buttons of the Variables section's header. The import steps apply
// a style sheet and ask for it a second time, so `finish` takes it back with one undo before the React page opens (the only steps of this script that change the file).
const THEME = ':root { --background: #ffffff; --foreground: #0a0a0a; --primary: var(--foreground); --radius: 8px; --shadow: 0 1px 2px rgb(0 0 0 / 0.1); }\n.dark { --background: #0a0a0a; --foreground: #fafafa; }\n@theme { --color-brand-500: oklch(0.6 0.2 250); --spacing-4: 1rem; }';
const filed = (p, name, bytes) => p.locator('[data-testid=import-vars-file]').setInputFiles({ name, mimeType: 'text/css', buffer: bytes });
const importing = async (p, text, status) => {
  await p.fill('[data-testid=import-vars-text]', text);
  await p.click('[data-testid=import-vars-import]');
  await p.locator(`[data-testid=import-vars-result][data-status=${status}]`).waitFor();
};
PANELS['io-export'] = {
  word: 'io',
  element: '[data-testid=export-vars-dialog]',
  steps: [
    { name: 'open (the summary, the preview, the help)', run: async (p) => { await p.click('[data-testid=var-export]'); await p.locator('[data-testid=export-vars-code]').waitFor(); } },
    { name: 'Copy pressed (what it said, whichever it was)', run: async (p) => { await p.click('[data-testid=export-vars-copy]'); await p.waitForFunction(() => document.querySelector('[data-testid=export-vars-said]')?.textContent !== ''); } },
  ],
};
PANELS['io-import'] = {
  word: 'io',
  element: '[data-testid=import-vars-dialog]',
  // The undo is local at once and reaches the server op by op: the next page must not open before the server has all of it.
  finish: async (p) => {
    await p.keyboard.press('Control+z');
    for (let i = 0; ; i++) {
      const res = await fetch(`${server}/v1/files/${file}`, { headers: { authorization: `Bearer ${token}` } });
      if ((await res.json()).variables?.collections?.length === 1) break;
      assert.ok(i < 100, 'the undo of the import did not reach the server');
      await sleep(200);
    }
  },
  steps: [
    { name: 'open, the field empty and focused', run: async (p) => { await p.click('[data-testid=var-import]'); await p.locator('[data-testid=import-vars-text]').waitFor(); } },
    { name: 'a text in the field (Import and Clear on)', run: (p) => p.fill('[data-testid=import-vars-text]', THEME) },
    { name: 'text with no variable: nothing to import, with its report', run: (p) => importing(p, '.card { --local: #ffffff; } body { color: #ff0000; }', 'nothing') },
    { name: 'a text of 51 variables, refused with the pointer to import_tokens', run: (p) => importing(p, `:root {${Array.from({ length: 51 }, (_, i) => ` --many-${i}: ${i}px;`).join('')} }`, 'refused') },
    { name: 'a file read into the field (the note says so)', run: async (p) => { await filed(p, 'theme.css', Buffer.from(THEME)); await p.waitForFunction(() => /^Read theme\.css/.test(document.querySelector('[data-testid=import-vars-note]')?.textContent ?? '')); } },
    { name: 'a file of more than 1 MiB, not read (the note in the error colour)', run: async (p) => { await filed(p, 'huge.css', Buffer.alloc(1024 * 1024 + 1, 32)); await p.waitForFunction(() => /not read here/.test(document.querySelector('[data-testid=import-vars-note]')?.textContent ?? '')); } },
    { name: 'the style sheet imported: what was added, the report with its kinds, the focus on Close', run: async (p) => { await p.fill('[data-testid=import-vars-text]', THEME); await p.click('[data-testid=import-vars-import]'); await p.locator('[data-testid=import-vars-result][data-status=imported]').waitFor(); } },
    { name: 'the same text again, refused as a duplicate', run: (p) => importing(p, THEME, 'refused') },
  ],
};

const browser = await chromium.launch({ args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });

async function pass(word, panel, react) {
  const ctx = await browser.newContext({ viewport: panel.viewport ?? { width: 1440, height: 900 }, deviceScaleFactor: 2, reducedMotion: 'reduce' });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  // A busy server (a loaded machine, a budget of calls used up) answers a 5xx (503 is what was seen) for a thumbnail, and the picker then images it as No preview on one
  // copy and not on the other, which is not a difference of the copies: the pass is made again (see `images` below) rather than compared.
  const refused = [];
  page.on('response', (r) => { if (r.status() >= 500 && /^\/v1\/assets\/[0-9a-f]{64}$/.test(new URL(r.url()).pathname)) refused.push(`${r.status()} ${new URL(r.url()).pathname.slice(-8)}`); });
  await page.goto(react ? `${web.replace(/\/?$/, '/')}?react=${word}` : web);
  await page.waitForSelector('[data-testid=connection][data-state=live]', { timeout: 20000 });
  await page.waitForSelector('[data-testid=layers]', { timeout: 20000 });
  await sleep(600);
  const out = [];
  for (const [i, step] of panel.steps.entries()) {
    await step.run(page);
    if (i === 0) {
      // The element may be there only after the first step (a popover): then ask who drew it.
      await page.waitForSelector(panel.element, { timeout: 20000 });
      assert.equal(await fibre(page, panel.element), react, `${word}: ${react ? 'React did not draw the panel (a fallback to Svelte)' : 'React drew the panel on the Svelte page'}`);
    }
    await sleep(500);
    await page.mouse.move(5, 5);
    await sleep(200);
    out.push((await page.locator(panel.element).first().screenshot()).toString('base64'));
  }
  await panel.finish?.(page);
  await ctx.close();
  assert.deepEqual(errors, [], `${word}: page errors`);
  if (refused.length) throw Object.assign(new Error(`${word}: the server refused a thumbnail (${refused.join(', ')})`), { refused: true });
  return out;
}

/** One pass over the steps; made again, up to twice, when the server refused a thumbnail (the steps only click and type, so a repeat sees the same file). */
async function images(word, panel, react) {
  for (let attempt = 1; ; attempt++) {
    try {
      return await pass(word, panel, react);
    } catch (e) {
      if (!e.refused || attempt === 3) throw e;
      console.log(`${e.message}: again (${attempt} of 3)`);
    }
  }
}

{
  // A file the server does not know yet is 404 and empty; any other answer that is not the file is an error.
  const res = await fetch(`${server}/v1/files/${file}`, { headers: { authorization: `Bearer ${token}` } });
  if (res.status !== 404) {
    assert.equal(res.status, 200, `reading the file: ${res.status}`);
    assert.equal((await res.json()).seq, 0, 'the file has content already: start the page and the check with a file name that is new');
  }
}
await post({ type: 'addNode', node: { id: 'f1', name: 'Frame one', type: 'frame', parentId: null, x: 0, y: 0, w: 400, h: 300, fill: '#ffffff' } });
await post({ type: 'addNode', node: { id: 'r1', name: 'Alpha', type: 'rect', parentId: 'f1', x: 10, y: 10, w: 80, h: 60, fill: '#335577' } });
await post({ type: 'addNode', node: { id: 'e1', name: 'Beta', type: 'ellipse', parentId: 'f1', x: 100, y: 10, w: 80, h: 60, fill: '#775533' } });
await post({ type: 'addNode', node: { id: 'l1', name: 'A layer whose name is far too long to fit in the row of the panel', type: 'rect', parentId: 'f1', x: 10, y: 140, w: 40, h: 20, fill: '#335577' } });
await post({ type: 'addCollection', collection: { id: 'c1', name: 'appearance', modes: [{ id: 'light', name: 'light' }, { id: 'dark', name: 'dark' }] } });
await post({ type: 'addVariable', variable: { id: 'v1', collection: 'c1', name: 'color/bg', type: 'color', values: { light: '#00aa55', dark: '#aa0055' } } });
await post({ type: 'addVariable', variable: { id: 'v2', collection: 'c1', name: 'color/ink', type: 'color', values: { light: '#111111', dark: '#eeeeee' } } });
await post({ type: 'addVariable', variable: { id: 'v3', collection: 'c1', name: 'space/gap', type: 'number', values: { light: 8, dark: 12 } } });
await post({ type: 'addNode', node: { id: 't1', name: 'A text', type: 'text', parentId: 'f1', x: 10, y: 100, w: 120, h: 30, text: 'Hello', fontSize: 16, fill: '#ffffff' } });

// The nodes of the Properties panel's states: a frame with a row layout, padding, a border and a shadow, a rectangle in it (the margin
// section), a rectangle with a gradient and one whose fill, opacity and corner radius are bound to variables.
await post({ type: 'addVariable', variable: { id: 'v4', collection: 'c1', name: 'opacity/half', type: 'number', values: { light: 0.5, dark: 0.75 } } });
await post({ type: 'addNode', node: { id: 'f2', name: 'Frame two', type: 'frame', parentId: null, x: 500, y: 0, w: 300, h: 200, fill: '#eeeeee' } });
await post({ type: 'updateNode', id: 'f2', patch: { layout: 'row', gap: 12, padding: 8, border: { width: 2, color: '#cc3333', position: 'outside' }, shadows: [{ x: 0, y: 4, blur: 8, spread: 0, color: '#00000040' }] } });
await post({ type: 'addNode', node: { id: 'r2', name: 'In the row', type: 'rect', parentId: 'f2', x: 0, y: 0, w: 60, h: 40, fill: '#335577' } });
await post({ type: 'updateNode', id: 'r2', patch: { marT: 6 } });
await post({ type: 'addNode', node: { id: 'g1', name: 'Gradient', type: 'rect', parentId: 'f1', x: 200, y: 140, w: 80, h: 60, fill: '#335577' } });
await post({ type: 'updateNode', id: 'g1', patch: { gradient: { kind: 'linear', angle: 90, stops: [{ pos: 0, color: '#335577' }, { pos: 0.5, color: '#aa5533' }, { pos: 1, color: '#ffffff' }] } } });
await post({ type: 'addNode', node: { id: 'g2', name: 'Radial', type: 'rect', parentId: 'f1', x: 100, y: 220, w: 80, h: 60, fill: '#335577' } });
await post({ type: 'updateNode', id: 'g2', patch: { gradient: { kind: 'radial', angle: 90, stops: [{ pos: 0, color: '#335577' }, { pos: 1, color: '#ffffff' }] } } });
await post({ type: 'addNode', node: { id: 'f3', name: 'Frame three', type: 'frame', parentId: null, x: 500, y: 300, w: 300, h: 200, fill: '#eeeeee' } });
const sh = (y) => ({ x: 0, y, blur: 4, spread: 0, color: '#00000040' });
await post({ type: 'updateNode', id: 'f3', patch: { layout: 'column', padding: 4, padL: 12, shadows: [sh(1), sh(2), sh(3), sh(4)] } });
await post({ type: 'addNode', node: { id: 'b1', name: 'Bound', type: 'rect', parentId: 'f1', x: 300, y: 140, w: 80, h: 60, fill: '#335577' } });
await post({ type: 'updateNode', id: 'b1', patch: { fill: { var: 'v1' }, opacity: { var: 'v4' }, radius: { var: 'v3' } } });

if (token2) await post({ type: 'addNode', node: { id: 'x1', name: 'From another actor', type: 'rect', parentId: 'f1', x: 60, y: 140, w: 40, h: 20, fill: '#775533' } }, token2);
else console.log('SKIP the marker of another actor in the Layers panel: MELDFRAME_E2E_TOKEN2 is not set (a second token with another actor among the server tokens)');

const differ = await makeDiffer(browser);

let bad = 0;
// MELDFRAME_PIXELS_ONLY=properties,picker compares those panels only (the file still gets every node, so the images are the same as in a full run).
const only = process.env.MELDFRAME_PIXELS_ONLY?.split(',');
// MELDFRAME_PIXELS_SHOTS=<an existing directory> saves the two images of every state there (`<panel>-<step>-svelte.png` and `-react.png`), to look at a difference.
const shots = process.env.MELDFRAME_PIXELS_SHOTS;
for (const [name, panel] of Object.entries(PANELS)) {
  if (only && !only.includes(name)) continue;
  const word = panel.word;
  await panel.before?.();
  const svelte = await images(word, panel, false);
  const react = await images(word, panel, true);
  for (let i = 0; i < panel.steps.length; i++) {
    if (shots) for (const [copy, img] of [['svelte', svelte[i]], ['react', react[i]]]) writeFileSync(`${shots}/${name}-${i + 1}-${copy}.png`, Buffer.from(img, 'base64'));
    const d = await differ(svelte[i], react[i]);
    const ok = !d.size && d.differing === 0;
    if (!ok) bad++;
    console.log(`${ok ? 'ok  ' : 'FAIL'} ${name}: ${panel.steps[i].name}: ${d.size ? `sizes differ ${d.size}` : `${d.differing} of ${d.total} pixels differ`}`);
  }
}
await browser.close();
console.log(bad ? `${bad} states differ` : 'pixels-e2e: ok');
process.exit(bad ? 1 : 0);
