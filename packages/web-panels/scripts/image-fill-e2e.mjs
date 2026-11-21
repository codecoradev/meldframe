// Browser check of image fill picking against a real server: node packages/web-panels/scripts/image-fill-e2e.mjs <web-url> <server-url>
// E2.5, ADR-0015 follow-up, decision 36: the Fill section's Image type opens a picker of the images the workspace holds (an
// agent uploads; the browser does not). The check uploads tiny PNGs to the workspace of the page's token through POST /v1/assets (the
// route the upload_asset tool of the agents shares), opens the picker, checks the tiles (thumbnail, short id, name), the
// keyboard (arrows, Escape, where focus goes), picks an image and checks the node's fill on the server, that the canvas draws it,
// the fit control, Change image, a thumbnail that fails to load and Retry previews, and that one undo undoes each choice; it ends with 52 more images to check Load more and where
// focus goes. A second workspace's token, when MELDFRAME_OTHER_TOKEN is set in the environment (a token of another workspace
// among the server's development tokens), must see none of the images by the list or by id; without it that check prints SKIP
// and counts as neither passed nor failed (the run ends with the number skipped). The empty-workspace and failed-list notes are checked by the component tests.
// Start the server and the page like the header of packages/web-variables/scripts/manage-e2e.mjs says: a file name that is new for every
// run (the check refuses a file that has content), the server on a free port, the development token of the page in .dev-token at the
// root, and the server's token list holding that token (and the other one, for the workspace check). Then give the page's URL and the
// server's URL as the two arguments and the file name in MELDFRAME_WEB_FILE: there are no defaults. The WASM bridge must be built
// from this revision (`just wasm-build`), and Playwright comes from the global module path. Both URLs must be loopback (the token is
// sent there). It adds a frame, two nodes and about 54 images to the file's workspace and leaves them: development server only.
// The checks run in order and share the file; one that fails can fail the ones after it.
// MELDFRAME_E2E_SHOTS=<dir> saves screenshots (and one of the page when a check fails).
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { deflateSync } from 'node:zlib';

const { chromium } = createRequire(import.meta.url)('playwright');
const [web, server] = process.argv.slice(2);
const root = new URL('../../../', import.meta.url).pathname;
const file = process.env.MELDFRAME_WEB_FILE;
const shots = process.env.MELDFRAME_E2E_SHOTS;
if (!web || !server || !file) throw new Error('usage: MELDFRAME_WEB_FILE=<a new file name> node image-fill-e2e.mjs <web-url> <server-url> (see the top of this file; no defaults)');
for (const u of [web, server]) {
  if (!['127.0.0.1', 'localhost', '[::1]'].includes(new URL(u).hostname)) throw new Error(`refusing to send the token to ${u}: the web and server URLs must be loopback (127.0.0.1, localhost or ::1)`);
}
const token = readFileSync(`${root}.dev-token`, 'utf8').trim();
const other = process.env.MELDFRAME_OTHER_TOKEN?.trim();
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
    body: JSON.stringify({ ops: [{ key: `if-${run}-${++n}`, command, deps: [] }] }),
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

const rgb = (hex) => [1, 3, 5].map((i) => Number.parseInt(hex.slice(i, i + 2), 16));
/** How many pixels of the screenshot are (nearly) this colour. */
async function count(hex) {
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

const crcTable = Array.from({ length: 256 }, (_, i) => { let c = i; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; return c >>> 0; });
const crc = (buf) => { let c = 0xffffffff; for (const b of buf) c = crcTable[(c ^ b) & 0xff] ^ (c >>> 8); return (c ^ 0xffffffff) >>> 0; };
/** A PNG of `w` x `h` pixels from `rowColour(y)` (a row is one colour). */
function pngOf(w, h, rowColour) {
  const chunk = (type, data) => {
    const len = Buffer.alloc(4); len.writeUInt32BE(data.length);
    const body = Buffer.concat([Buffer.from(type), data]);
    const sum = Buffer.alloc(4); sum.writeUInt32BE(crc(body));
    return Buffer.concat([len, body, sum]);
  };
  const head = Buffer.alloc(13); head.writeUInt32BE(w, 0); head.writeUInt32BE(h, 4); head[8] = 8; head[9] = 2;
  const raw = Buffer.alloc((w * 3 + 1) * h);
  for (let y = 0; y < h; y++) { const o = y * (w * 3 + 1); raw[o] = 0; const [r, g, b] = rgb(rowColour(y)); for (let x = 0; x < w; x++) { raw[o + 1 + x * 3] = r; raw[o + 2 + x * 3] = g; raw[o + 3 + x * 3] = b; } }
  return Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk('IHDR', head), chunk('IDAT', deflateSync(raw)), chunk('IEND', Buffer.alloc(0))]);
}
/** Uploads an image to the workspace of `who` and returns its id. */
async function upload(bytes, who = token) {
  const res = await fetch(`${server}/v1/assets`, { method: 'POST', headers: { authorization: `Bearer ${who}` }, body: bytes });
  const body = await res.json();
  assert.equal(res.status, 200, `upload refused: ${res.status} ${JSON.stringify(body)}`);
  assert.equal(body.id, createHash('sha256').update(bytes).digest('hex'));
  return body.id;
}
const listOf = async (who, query = '') => {
  const res = await fetch(`${server}/v1/assets${query}`, { headers: { authorization: `Bearer ${who}` } });
  return { status: res.status, body: await res.json() };
};

const $ = (sel) => page.locator(sel);
const activeDataId = () => page.evaluate(() => document.activeElement?.getAttribute('data-id') ?? '');
const results = [];
/** A check that did not run: printed as SKIP, and counted as neither passed nor failed. */
function skip(name, why) {
  results.push(null);
  console.log(`SKIP ${name}: ${why}`);
}
async function check(name, fn) {
  try {
    await fn();
    results.push(true);
    console.log(`ok   ${name}`);
  } catch (e) {
    results.push(false);
    if (shots && page) await page.screenshot({ path: `${shots}/image-fill-fail-${results.length}.png` }).catch(() => {});
    console.log(`FAIL ${name}: ${String(e.message ?? e).split('\n').slice(0, process.env.MELDFRAME_E2E_DEBUG ? 12 : 1).join(' | ')}`);
  }
}
const shot = (name) => (shots && page ? page.screenshot({ path: `${shots}/image-fill-${name}.png` }) : null);

const frame = `frame_${run}`;
const rect = `rect_${run}`;
const label = `text_${run}`;
const S = ['#e91e63', '#00bcd4', '#8bc34a']; // image A: stripes, top to bottom
const T = ['#ff9800', '#673ab7', '#009688']; // image B
const short = (id) => id.slice(0, 8);
const imageRadio = () => $('[data-testid=prop-fill-type]').getByRole('radio', { name: 'Image', exact: true });
// The panel shows the node once its name is in the name field (the panel of the node selected before stays up until then).
const select = async (id, name) => {
  await $(`[data-testid=layer][data-id="${id}"] button`).last().click();
  await until('the properties panel shows the node', async () => (await $('[data-testid=prop-name]').inputValue().catch(() => '')) === name);
};

(async () => {
  assert.equal((await snapshot()).seq, 0, 'the file has content already: start the page and the check with a file name that is new');
  await post({ type: 'addNode', node: { id: frame, name: `Card ${run}`, type: 'frame', parentId: null, x: 0, y: 0, w: 300, h: 200, fill: '#ffffff' } });
  await post({ type: 'addNode', node: { id: rect, name: `Box ${run}`, type: 'rect', parentId: frame, x: 10, y: 10, w: 120, h: 60, fill: '#204060' } });
  await post({ type: 'addNode', node: { id: label, name: `Words ${run}`, type: 'text', parentId: frame, x: 10, y: 100, w: 120, h: 30, fill: '#000000', text: 'Hello' } });
  const A = await upload(pngOf(20, 20, (y) => (y < 7 ? S[0] : y < 14 ? S[1] : S[2])));
  const B = await upload(pngOf(20, 20, (y) => (y < 7 ? T[0] : y < 14 ? T[1] : T[2])));
  const [first, second] = [A, B].sort();
  browser = await chromium.launch({ args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
  page = await browser.newPage({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1 });
  page.on('pageerror', (e) => { pageErrors.push(e.message); console.log('[pageerror]', e.message); });
  await page.goto(web);
  await until('the editor is live', async () => (await page.getAttribute('[data-testid=connection]', 'data-state')) === 'live');
  await until('the nodes are listed', async () => (await $(`[data-testid=layer][data-id="${rect}"]`).count()) === 1);

  await check('the list route gives the two images by id, with size and type, and no content', async () => {
    const { status, body } = await listOf(token);
    assert.equal(status, 200);
    assert.deepEqual(body.assets.map((a) => a.id), [first, second]);
    assert.ok(body.assets.every((a) => a.mime === 'image/png' && a.bytes > 0 && Object.keys(a).sort().join() === 'bytes,id,mime'));
    assert.equal(body.next, null);
  });

  await check('the Image type is offered to a rectangle and not to text', async () => {
    await select(label, `Words ${run}`);
    assert.equal(await imageRadio().count(), 0, 'text takes no image');
    await select(rect, `Box ${run}`);
    assert.equal(await imageRadio().count(), 1);
    assert.equal(await $('[data-testid=image-picker]').count(), 0, 'the picker is closed until the person asks');
  });

  await check('choosing Image opens the picker with the two images, the short id as the label, thumbnails that loaded, and the focus on the first tile', async () => {
    await imageRadio().click();
    await $('[data-testid=image-picker]').waitFor();
    await until('both tiles', async () => (await $('[data-testid=image-tile]').count()) === 2);
    assert.deepEqual(await $('[data-testid=image-tile]').evaluateAll((t) => t.map((x) => x.getAttribute('data-id'))), [first, second]);
    for (const id of [first, second]) {
      const tile = $(`[data-testid=image-tile][data-id="${id}"]`);
      assert.match((await tile.textContent()) ?? '', new RegExp(short(id)));
      assert.match((await tile.getAttribute('aria-label')) ?? '', new RegExp(`^Image ${short(id)}, PNG, \\d+ B$`));
      await until('the thumbnail loaded', () => tile.locator('img').evaluate((i) => i.complete && i.naturalWidth > 0));
    }
    assert.equal(await $('[data-testid=prop-fill-type] [aria-checked=true]').textContent().then((t) => t.trim()), 'Solid', 'the fill is not changed by opening');
    await until('the focus is on the first tile', async () => (await activeDataId()) === first);
    assert.equal(await $('[data-testid=image-picker-empty]').count(), 0);
    await shot('picker');
  });

  await check('the arrow keys move the focus over the tiles, the grid is one tab stop, and Escape closes the picker and puts the focus back on the switch', async () => {
    await page.keyboard.press('ArrowRight');
    await until('the second tile', async () => (await activeDataId()) === second);
    await page.keyboard.press('ArrowRight');
    assert.equal(await activeDataId(), second, 'the last tile stays');
    await page.keyboard.press('Home');
    assert.equal(await activeDataId(), first);
    assert.deepEqual(await $('[data-testid=image-tile]').evaluateAll((t) => t.map((x) => x.tabIndex)), [0, -1]);
    await page.keyboard.press('Escape');
    await until('the picker is closed', async () => (await $('[data-testid=image-picker]').count()) === 0);
    await until('the focus is on the checked type', async () => page.evaluate(() => document.activeElement?.getAttribute('role') === 'radio' && document.activeElement?.getAttribute('aria-checked') === 'true'));
    assert.equal(await (await nodeOf(rect)).image, undefined, 'nothing was written');
  });

  await check('picking an image sets the fill of the node with the fit cover, closes the picker and draws it on the canvas', async () => {
    await imageRadio().click();
    await $(`[data-testid=image-tile][data-id="${A}"]`).click();
    const node = await until('the image on the server', async () => (await nodeOf(rect)).image);
    assert.deepEqual(node, { assetId: A, fit: 'cover' });
    await until('the picker is closed', async () => (await $('[data-testid=image-picker]').count()) === 0);
    await until('the Image type is checked', async () => (await $('[data-testid=prop-fill-type] [aria-checked=true]').textContent())?.trim() === 'Image');
    assert.match((await $('[data-testid=prop-fill-image]').textContent()) ?? '', new RegExp(`Image ${short(A)}`));
    await until('the stripes are drawn', async () => (await count(S[0])) > 50);
    await until('the focus is back on the switch', async () => page.evaluate(() => document.activeElement?.getAttribute('role') === 'radio'));
    await shot('picked');
  });

  await check('the fit control changes the fit in one step, and one undo takes it back', async () => {
    await $('[data-testid=prop-fill-image-fit]').getByRole('radio', { name: 'Contain', exact: true }).click();
    await until('contain on the server', async () => (await nodeOf(rect)).image.fit === 'contain');
    assert.equal((await nodeOf(rect)).image.assetId, A);
    await $('[data-testid=undo]').click();
    await until('cover again', async () => (await nodeOf(rect)).image.fit === 'cover');
    assert.ok((await $('[data-testid=prop-fill-image-fit] [aria-checked=true]').textContent())?.includes('Cover'));
  });

  await check('Change image marks the current image, picks the other one with the fit kept, and one undo brings the first back', async () => {
    await $('[data-testid=prop-fill-image-fit]').getByRole('radio', { name: 'Fill', exact: true }).click();
    await until('fill on the server', async () => (await nodeOf(rect)).image.fit === 'fill');
    await $('[data-testid=prop-fill-image-change]').click();
    await $('[data-testid=image-picker]').waitFor();
    await until('both tiles', async () => (await $('[data-testid=image-tile]').count()) === 2);
    assert.equal(await $(`[data-testid=image-tile][data-id="${A}"]`).getAttribute('aria-current'), 'true');
    assert.equal(await $(`[data-testid=image-tile][data-id="${B}"]`).getAttribute('aria-current'), null);
    assert.equal(await $('[data-testid=image-tile][aria-pressed]').count(), 0, 'a tile is not a toggle');
    await $(`[data-testid=image-tile][data-id="${B}"]`).click();
    await until('the other image on the server', async () => (await nodeOf(rect)).image.assetId === B);
    assert.equal((await nodeOf(rect)).image.fit, 'fill', 'the fit the node had is kept');
    await until('the new stripes are drawn and the old ones are gone', async () => (await count(T[0])) > 50 && (await count(S[0])) < 5);
    await $('[data-testid=undo]').click();
    await until('the first image back', async () => (await nodeOf(rect)).image.assetId === A);
    await until('its stripes are drawn again', async () => (await count(S[0])) > 50);
  });

  await check('the picker closes when another node is selected, and the node keeps nothing it was not given', async () => {
    await $('[data-testid=prop-fill-image-change]').click();
    await $('[data-testid=image-picker]').waitFor();
    await select(frame, `Card ${run}`);
    await until('the picker is gone', async () => (await $('[data-testid=image-picker]').count()) === 0);
    assert.equal((await nodeOf(frame)).image, undefined);
    await select(rect, `Box ${run}`);
  });

  await check('a thumbnail that failed to load says so, and Retry previews asks for it again', async () => {
    const thumb = new RegExp(`/v1/assets/${B}$`);
    await page.route(thumb, (r) => r.abort());
    await $('[data-testid=prop-fill-image-change]').click();
    await $('[data-testid=image-picker]').waitFor();
    await until('both tiles', async () => (await $('[data-testid=image-tile]').count()) === 2);
    await until('the preview is said not to have loaded', async () => (await $(`[data-testid=image-tile][data-id="${B}"]`).textContent())?.includes('No preview'));
    assert.match((await $('[data-testid=image-picker-broken]').textContent()) ?? '', /1 preview did not load/);
    assert.equal(await $(`[data-testid=image-tile][data-id="${A}"] img`).count(), 1, 'the other tile keeps its image');
    await page.unroute(thumb);
    await $('[data-testid=image-picker-retry-previews]').click();
    const img = $(`[data-testid=image-tile][data-id="${B}"] img`);
    await until('the preview loads now', async () => (await img.count()) === 1 && (await img.evaluate((i) => i.complete && i.naturalWidth > 0)));
    assert.equal(await $('[data-testid=image-picker-broken]').count(), 0);
    await until('the focus is on a tile, not lost with the button', async () => (await $('[data-testid=image-picker]').evaluate((p) => p.contains(document.activeElement) && document.activeElement !== p)));
    await page.keyboard.press('Escape');
    await until('the picker is closed', async () => (await $('[data-testid=image-picker]').count()) === 0);
  });

  await check('a failed list says so, Try again asks again, and the focus stays inside the picker so Escape still closes it', async () => {
    const list = /\/v1\/assets\?/;
    await page.route(list, (r) => r.fulfill({ status: 503, contentType: 'application/json', body: JSON.stringify({ error: { code: 'unavailable', message: 'busy', retryable: true } }) }));
    await $('[data-testid=prop-fill-image-change]').click();
    await $('[data-testid=image-picker-error]').waitFor();
    assert.match((await $('[data-testid=image-picker-error]').textContent()) ?? '', /unavailable/);
    await page.unroute(list);
    await $('[data-testid=image-picker-retry]').click();
    await until('the list is read', async () => (await $('[data-testid=image-tile]').count()) === 2);
    assert.equal(await $('[data-testid=image-picker-error]').count(), 0);
    await until('the focus is inside the picker, not on the page', async () => (await $('[data-testid=image-picker]').evaluate((p) => p.contains(document.activeElement))));
    await page.keyboard.press('Escape');
    await until('the picker is closed', async () => (await $('[data-testid=image-picker]').count()) === 0);
  });

  if (!other) skip('a second workspace sees none of the images, by the list or by id', 'MELDFRAME_OTHER_TOKEN is not set (a token of a second workspace among the server tokens)');
  else await check('a second workspace sees none of the images, by the list or by id', async () => {
    const { status, body } = await listOf(other);
    assert.equal(status, 200);
    assert.deepEqual(body.assets, []);
    const res = await fetch(`${server}/v1/assets/${A}`, { headers: { authorization: `Bearer ${other}` } });
    assert.equal(res.status, 404);
    assert.equal((await res.json()).error.code, 'asset_not_found');
  });

  await check('with more than a page of images, Load more reads the next page and the focus goes to its first new tile', async () => {
    const ids = [first, second];
    for (let i = 0; i < 52; i++) ids.push(await upload(pngOf(4, 4, () => `#${(0x100000 + i * 4099).toString(16).padStart(6, '0')}`)));
    ids.sort();
    // The server allows a burst of 60 calls from one address (then 30 a second): let the uploads' budget fill again before the
    // picker asks for a page of thumbnails.
    await sleep(3000);
    const tiles = () => $('[data-testid=image-tile]').evaluateAll((t) => t.map((x) => x.getAttribute('data-id')));
    await $('[data-testid=prop-fill-image-change]').click();
    await $('[data-testid=image-picker]').waitFor();
    await until('the first page of 24', async () => (await $('[data-testid=image-tile]').count()) === 24);
    assert.deepEqual(await tiles(), ids.slice(0, 24));
    // Keyboard: focus the button, press Enter; the focus goes to the first tile of the page that comes.
    await $('[data-testid=image-picker-more]').focus();
    await page.keyboard.press('Enter');
    await until('48 tiles, in the order of the ids', async () => (await $('[data-testid=image-tile]').count()) === 48);
    assert.deepEqual(await tiles(), ids.slice(0, 48));
    await until('the focus is on the first new tile', async () => (await activeDataId()) === ids[24]);
    await sleep(1500);
    await $('[data-testid=image-picker-more]').click();
    await until('all 54 tiles, in the order of the ids', async () => (await $('[data-testid=image-tile]').count()) === 54);
    assert.deepEqual(await tiles(), ids);
    assert.equal(await $('[data-testid=image-picker-more]').count(), 0, 'no more to read');
    await until('the focus is on the first new tile of the last page', async () => (await activeDataId()) === ids[48]);
    await page.keyboard.press('Escape');
    await until('the picker is closed', async () => (await $('[data-testid=image-picker]').count()) === 0);
  });

  const failed = results.filter((r) => r === false).length;
  console.log(`${results.filter((r) => r === true).length} passed, ${failed} failed, ${results.filter((r) => r === null).length} skipped`);
  console.log(pageErrors.length ? `${pageErrors.length} page error(s)` : 'no page errors');
  await browser.close();
  process.exit(failed || pageErrors.length ? 1 : 0);
})().catch(async (e) => {
  console.log('ERROR', e);
  await browser?.close().catch(() => {});
  process.exit(1);
});
