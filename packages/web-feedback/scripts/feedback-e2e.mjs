// Browser check of what the editor says about itself: node packages/web-feedback/scripts/feedback-e2e.mjs <web-url> <server-url>
// The banners over the canvas (a connection that is lost, one that is down, a page that no longer agrees with the file, a
// canvas that cannot start), the toast, the empty state of a file with no frame, and the error page of a file that did not open.
// The same script runs on both copies: MELDFRAME_UI=react opens the page with `react=feedback` (the React copy of
// packages/web-feedback, E7.3 slice S3); without it the Svelte copy. `data-testid` values, roles and names are the contract.
// Start the server and the page: a file name that is new for every run (the check refuses a file that has content), the server
// on a free port with the token of the page in .dev-token at the root among its development tokens (MELDFRAME_DEV_TOKENS), the
// page on Vite with MELDFRAME_WEB_TOKEN (that token), MELDFRAME_SERVER (the server) and MELDFRAME_WEB_FILE (the file name),
// and the WASM bridge built from this revision (`just wasm-build`). Give the two URLs as arguments and the file name in
// MELDFRAME_WEB_FILE: there are no defaults. Both URLs must be loopback (the token is sent there). Playwright comes from the
// global module path. It adds a rect and a frame to the file and leaves them: development server only. The checks run in order
// and share the file; one that fails can fail the ones after it. The server is never stopped: the socket of the page is cut
// and refused with Playwright's WebSocket routes, so no process needs to be restarted.
// How each state is made: connection banners by cutting the live socket and refusing the next ones; the page-and-server
// banner by an event frame that no longer fits the file, sent over that socket (with the automatic reload used up); the canvas
// banner by a browser whose canvases have no drawing context; the toast by a node whose image the server does not hold (the editor says `asset_not_found`
// once); the empty state by a file with no node; the error page by answers made up for the snapshot request, and by a WASM
// module that does not load.
// MELDFRAME_E2E_SHOTS=<dir> saves a screenshot of each state there (feedback-<state>-<svelte|react>.png, 1440x900 at scale 2,
// reduced motion), for a pixel comparison of the two copies.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';

const { chromium } = createRequire(import.meta.url)('playwright');
const [web, server] = process.argv.slice(2);
const root = new URL('../../../', import.meta.url).pathname;
const file = process.env.MELDFRAME_WEB_FILE;
const shots = process.env.MELDFRAME_E2E_SHOTS;
const react = process.env.MELDFRAME_UI === 'react';
const ui = react ? 'react' : 'svelte';
if (!web || !server || !file) throw new Error('usage: MELDFRAME_WEB_FILE=<a new file name> node feedback-e2e.mjs <web-url> <server-url> (see the top of this file; no defaults)');
for (const u of [web, server]) {
  if (!['127.0.0.1', 'localhost', '[::1]'].includes(new URL(u).hostname)) throw new Error(`refusing to send the token to ${u}: the web and server URLs must be loopback (127.0.0.1, localhost or ::1)`);
}
const token = readFileSync(`${root}.dev-token`, 'utf8').trim();
const run = Math.random().toString(36).slice(2, 7);
const page_url = (extra = '') => `${web}/?${[react ? 'react=feedback' : '', extra].filter(Boolean).join('&')}`;
const pageErrors = [];
const results = [];
let browser;
let n = 0;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function until(what, fn, ms = 20000) {
  const t0 = Date.now();
  for (;;) {
    const v = await fn();
    if (v) return v;
    if (Date.now() - t0 > ms) throw new Error(`timed out waiting for: ${what}`);
    await sleep(100);
  }
}
async function check(name, fn) {
  try { await fn(); results.push(true); console.log('ok  ', name); }
  catch (e) { results.push(false); console.log('FAIL', name, '\n    ', String(e.message ?? e).split('\n').slice(0, 5).join(' / ')); }
}
async function post(command) {
  const res = await fetch(`${server}/v1/files/${file}/tools/apply_commands`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', authorization: `Bearer ${token}` },
    body: JSON.stringify({ ops: [{ key: `fb-${run}-${++n}`, command, deps: [] }] }),
  });
  assert.equal(res.status, 200, `op refused: ${res.status} ${await res.text()}`);
}
async function newPage(init, track = false) {
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2, reducedMotion: 'reduce' });
  const page = await context.newPage();
  if (track) page.on('pageerror', (e) => { pageErrors.push(e.message); console.log('[pageerror]', e.message); });
  if (init) await page.addInitScript(init);
  return page;
}
const shot = async (page, state) => { if (shots) await page.screenshot({ path: `${shots}/feedback-${state}-${ui}.png` }); };
const state = (page) => page.getAttribute('[data-testid=connection]', 'data-state');
/** True when the element carries a React fiber: the guard against a page that says "React" and draws Svelte. */
// The modules of the React copy that Vite or the built site loads on demand.
const chunk = /FeedbackHost|FeedbackLayerHost|react\/ErrorPage|react\/FeedbackLayer/;
const fibre = (page, selector) => page.evaluate((s) => Object.keys(document.querySelector(s) ?? {}).some((k) => k.startsWith('__reactFiber$')), selector);

/**
 * The live socket of the page (`/v1/live/<file>`), between the page and the server. `mode` is what the next socket gets:
 * `open` goes through to the server, `hold` is accepted and never answered (the attempt hangs, so the page says Reconnecting),
 * `refuse` is closed at once (the attempt fails, so the page says Offline). `cut` closes the one that is open and holds the next.
 */
function socketControl() {
  const c = { mode: 'open', current: null, room: null, seq: 0, fromServer: null };
  c.route = (page) =>
    page.routeWebSocket(/\/v1\/live\//, (ws) => {
      if (c.mode === 'refuse') return void ws.close();
      if (c.mode === 'hold') return;
      const upstream = ws.connectToServer();
      c.current = ws;
      c.fromServer = (msg) => ws.send(msg);
      ws.onMessage((m) => upstream.send(m));
      upstream.onMessage((m) => {
        try {
          const f = JSON.parse(String(m));
          if (f.type === 'event') c.seq = Math.max(c.seq, f.seq);
          if (f.room) c.room = f.room;
        } catch { /* not JSON: pass it on */ }
        ws.send(m);
      });
    });
  c.cut = () => { c.mode = 'hold'; c.current?.close(); };
  return c;
}

const alertRegion = '[data-testid=banner-region-alert]';
const statusRegion = '[data-testid=banner-region-status]';

(async () => {
  browser = await chromium.launch({ args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
  // The reload that follows a divergence is used up, so the page shows its banner instead of reloading (see reloadOnDivergence).
  const noReload = () => sessionStorage.setItem('meldframe-reloads', JSON.stringify({ count: 3, at: Date.now() }));
  const page = await newPage(noReload, true);
  const ws = socketControl();
  await ws.route(page);
  await page.goto(page_url());

  await check(`the page draws the feedback with ${react ? 'React' : 'Svelte'}`, async () => {
    await until('connected', async () => (await state(page)) === 'live');
    await page.locator('[data-testid=toast-region]').waitFor({ state: 'attached' });
    assert.equal(await fibre(page, '[data-testid=toast-region]'), react, react ? 'the toast region has no React fiber: the page fell back to Svelte' : 'the toast region has a React fiber');
    assert.equal(await fibre(page, '[data-testid=banner-region-alert]'), react);
  });

  await check('a file with no frame says so over the canvas, with one button, and leaves the canvas under it to the pointer', async () => {
    const card = page.locator('[data-testid=empty-state]');
    await card.waitFor({ state: 'visible', timeout: 15000 });
    assert.equal(await card.getAttribute('aria-label'), 'No frames yet');
    assert.equal(await card.locator('h2').textContent(), 'No frames yet');
    assert.match(await card.locator('p').textContent(), /^Press F and drag to draw a frame/);
    assert.equal(await card.locator('button').count(), 1);
    const add = card.getByRole('button', { name: 'Add a frame' });
    assert.equal(await add.isEnabled(), true);
    // The container around the card takes no pointer events (the canvas under it still works); the card does.
    const pe = await card.evaluate((el) => [getComputedStyle(el.parentElement).pointerEvents, getComputedStyle(el).pointerEvents, getComputedStyle(el.parentElement).position]);
    assert.deepEqual(pe, ['none', 'auto', 'absolute']);
    const box = await card.boundingBox();
    const viewport = page.viewportSize();
    assert.ok(Math.abs(box.x + box.width / 2 - (await page.locator('canvas').first().boundingBox().then((b) => b.x + b.width / 2))) < 2, 'the card is centred on the canvas');
    assert.ok(box.width <= 320 && box.y > 0 && box.y + box.height < viewport.height, `the card is a small card: ${JSON.stringify(box)}`);
    await shot(page, 'no-frames');
  });

  if (react) {
    await check('React chunk of the editor layer that does not load: the empty state is drawn by Svelte', async () => {
      const p = await newPage();
      await p.route((u) => chunk.test(u.pathname), (route) => route.abort('failed'));
      await p.goto(page_url());
      const card = p.locator('[data-testid=empty-state]');
      await card.waitFor({ state: 'visible', timeout: 20000 });
      assert.equal(await fibre(p, '[data-testid=empty-state]'), false, 'the empty state is the Svelte copy');
      await p.context().close();
    });
  }

  await check('a node whose image the server does not hold shows a toast in a live region, with the tone and a Dismiss button', async () => {
    await post({ type: 'addNode', node: { id: `${run}_img1`, name: 'Image one', type: 'rect', parentId: null, x: 0, y: 0, w: 120, h: 60, fill: '#a0522d', image: { assetId: 'e'.repeat(64), fit: 'cover' } } });
    const region = page.locator('[data-testid=toast-region]');
    const notice = region.locator('[data-testid=notice]');
    await notice.waitFor({ state: 'visible', timeout: 10000 });
    assert.equal(await region.getAttribute('role'), 'status');
    assert.equal(await notice.getAttribute('data-tone'), 'warning');
    assert.match(await notice.textContent(), /An image could not be loaded \(asset_not_found\)\./);
    assert.equal(await notice.locator('button').getAttribute('aria-label'), 'Dismiss');
    const look = await notice.evaluate((el) => [getComputedStyle(el).pointerEvents, getComputedStyle(el.parentElement).pointerEvents]);
    assert.deepEqual(look, ['auto', 'none']);
    await shot(page, 'toast');
  });

  await check('a second message with the same text is a new element, so it is announced again; Dismiss takes it away; it goes away by itself', async () => {
    const notice = page.locator('[data-testid=notice]');
    await notice.evaluate((el) => { window.__first = el; });
    await post({ type: 'addNode', node: { id: `${run}_img2`, name: 'Image two', type: 'rect', parentId: null, x: 130, y: 0, w: 120, h: 60, fill: '#a0522d', image: { assetId: 'd'.repeat(64), fit: 'cover' } } });
    await until('a new element for the same text', () => page.evaluate(() => { const el = document.querySelector('[data-testid=notice]'); return el && el !== window.__first && !window.__first.isConnected; }), 10000);
    await page.locator('[data-testid=notice]').getByRole('button', { name: 'Dismiss' }).click();
    await page.locator('[data-testid=notice]').waitFor({ state: 'detached', timeout: 3000 });
    assert.equal(await page.locator('[data-testid=toast-region]').count(), 1, 'the live region stays on the page with nothing in it');
    await post({ type: 'addNode', node: { id: `${run}_img3`, name: 'Image three', type: 'rect', parentId: null, x: 260, y: 0, w: 120, h: 60, fill: '#a0522d', image: { assetId: 'c'.repeat(64), fit: 'cover' } } });
    await page.locator('[data-testid=notice]').waitFor({ state: 'visible', timeout: 10000 });
    await page.locator('[data-testid=notice]').waitFor({ state: 'detached', timeout: 12000 });
  });

  await check('a page that no longer agrees with the file shows an error banner in the alert region with Reload, and Add a frame is off until the page is loaded again', async () => {
    assert.ok(ws.room && ws.seq > 0, 'the page has seen the room and an event');
    ws.fromServer(JSON.stringify({ type: 'event', room: ws.room, seq: ws.seq + 1, actor: 'agent:other', command: { type: 'updateNode', id: `${run}_nope`, patch: { fill: '#112233' } } }));
    const banner = page.locator(`${alertRegion} [data-testid=banner-halted]`);
    await banner.waitFor({ state: 'visible', timeout: 10000 });
    assert.equal(await banner.getAttribute('data-tone'), 'danger');
    assert.match(await banner.textContent(), /^This page and the server no longer agree about the file\./);
    assert.equal(await banner.getByRole('button', { name: 'Reload' }).count(), 1);
    assert.equal(await page.locator(`${statusRegion} [data-testid=banner-halted]`).count(), 0);
    assert.equal(await state(page), 'halted');
    // The file has no frame yet: the empty state stays, and its button is there but off while the page is out of step.
    assert.equal(await page.locator('[data-testid=empty-state]').getByRole('button', { name: 'Add a frame' }).isDisabled(), true);
    await shot(page, 'banner-halted');
    // Reload goes to the server for the file again: the page is drawn again and the banner goes.
    await Promise.all([page.waitForNavigation({ timeout: 15000 }), banner.getByRole('button', { name: 'Reload' }).click()]);
    await until('live after the reload', async () => (await state(page).catch(() => null)) === 'live', 30000);
    assert.equal(await page.locator('[data-testid=banner-halted]').count(), 0);
    await page.locator('[data-testid=banner-region-alert]').waitFor({ state: 'attached' }); // the React layer is loaded on demand
    assert.equal(await fibre(page, '[data-testid=banner-region-alert]'), react);
    assert.equal(await page.locator('[data-testid=empty-state]').getByRole('button', { name: 'Add a frame' }).isEnabled(), true);
  });

  await check('Add a frame makes a frame and the empty state goes', async () => {
    await page.locator('[data-testid=empty-state]').getByRole('button', { name: 'Add a frame' }).click();
    await page.locator('[data-testid=empty-state]').waitFor({ state: 'detached', timeout: 8000 });
    const nodes = await (await fetch(`${server}/v1/files/${file}`, { headers: { authorization: `Bearer ${token}` } })).json();
    assert.ok(nodes.nodes.some((x) => x.type === 'frame'), 'the frame is on the server');
  });

  await check('the connection is lost: warning banners in the status region, none in the alert region; Offline has Retry now, Reconnecting has no button', async () => {
    ws.cut();
    const banner = page.locator(`${statusRegion} [data-testid=banner-connection]`);
    await banner.waitFor({ state: 'visible', timeout: 15000 });
    assert.equal(await banner.getAttribute('data-tone'), 'warning');
    assert.equal(await page.locator(statusRegion).getAttribute('role'), 'status');
    assert.equal(await page.locator(alertRegion).getAttribute('role'), 'alert');
    assert.equal(await page.locator(`${alertRegion} [data-testid^=banner-]`).count(), 0, 'no banner in the alert region');
    // The page goes offline at once, waits the backoff, then tries again: that attempt hangs, and the page says Reconnecting.
    await until('the attempt that hangs', async () => /^Reconnecting to the server\. Attempt 1\./.test((await banner.textContent()) ?? ''), 30000);
    assert.equal(await banner.locator('button').count(), 0, 'no action while it reconnects');
    assert.deepEqual(await banner.evaluate((el) => [getComputedStyle(el).pointerEvents, getComputedStyle(el.parentElement.parentElement).pointerEvents]), ['auto', 'none']);
    await shot(page, 'banner-reconnecting');
    // The next attempt is refused at once: Offline, with the one button.
    ws.mode = 'refuse';
    const retry = banner.getByRole('button', { name: 'Retry now' });
    await retry.waitFor({ state: 'visible', timeout: 90000 });
    assert.match(await banner.textContent(), /^Offline\. Can't reach the server\./);
    assert.equal(await state(page), 'offline');
    await shot(page, 'banner-offline');
    // Retry while the server is still refused: the banner stays.
    await retry.click();
    await retry.waitFor({ state: 'visible', timeout: 10000 });
  });

  await check('the connection is back: Retry now reconnects, and the banner goes', async () => {
    ws.mode = 'open';
    const retry = page.locator('[data-testid=banner-connection]').getByRole('button', { name: 'Retry now' });
    if (await retry.count()) await retry.click();
    await until('live again', async () => (await state(page)) === 'live', 60000);
    await page.locator('[data-testid=banner-connection]').waitFor({ state: 'detached', timeout: 5000 });
  });

  // A browser whose canvases have no context (no WebGL, no 2d, no WebGPU): the canvas cannot start.
  await check('a canvas that cannot start says so in an error banner, over the page', async () => {
    const noGl = () => {
      HTMLCanvasElement.prototype.getContext = function () { return null; };
      Object.defineProperty(navigator, 'gpu', { value: undefined, configurable: true });
    };
    const bare = await newPage(noGl);
    await bare.goto(page_url());
    const banner = bare.locator(`${alertRegion} [data-testid=banner-canvas-error]`);
    await banner.waitFor({ state: 'visible', timeout: 20000 });
    assert.equal(await banner.getAttribute('data-tone'), 'danger');
    assert.match(await banner.textContent(), /^The canvas could not start: /);
    assert.equal(await banner.locator('button').count(), 0);
    await shot(bare, 'banner-canvas-error');
    await bare.context().close();
  });

  // The error page: answers made up for the snapshot request; the page asks again on the button.
  const answers = [
    ['no-access', { status: 403, body: 'forbidden: <b>no</b>' }, /^No access to this file$/, /The server answered 403: forbidden: <b>no<\/b>/],
    ['not-accepted', { status: 401, body: 'unauthorized' }, /^The server did not accept this page$/, /The server answered 401: unauthorized/],
    ['server', { status: 500, body: '' }, /^The server could not open the file$/, /The server answered 500\./],
    ['unreachable', 'abort', /^Can't reach the server$/, null],
  ];
  for (const [kind, answer, title, detail] of answers) {
    await check(`the error page for a file that did not open (${kind}): the main landmark, an alert with the heading focused, the detail as text, Try again asks again`, async () => {
      const p = await newPage();
      let asked = 0;
      await p.route((u) => u.pathname === `/v1/files/${file}`, (route) => {
        asked++;
        return answer === 'abort' ? route.abort('failed') : route.fulfill({ status: answer.status, contentType: 'text/plain', body: answer.body });
      });
      await p.goto(page_url());
      const err = p.locator('[data-testid=error-page]');
      await err.waitFor({ state: 'visible', timeout: 20000 });
      assert.equal(await err.getAttribute('data-kind'), kind);
      assert.equal(await err.evaluate((el) => el.tagName), 'MAIN');
      assert.match(await err.locator('h1').textContent(), title);
      assert.equal(await err.locator('[role=alert] h1').count(), 1, 'the heading is in the alert');
      assert.equal(await p.evaluate(() => document.activeElement?.tagName), 'H1', 'the focus starts at the heading');
      if (detail) {
        const text = await err.locator('[data-testid=error-detail]').textContent();
        assert.match(text, detail);
        assert.equal(await err.locator('[data-testid=error-detail] b').count(), 0, 'the detail is text, not markup');
      } else assert.equal(await err.locator('[data-testid=error-detail]').count(), 0);
      assert.equal(await fibre(p, '[data-testid=error-page]'), react, react ? 'the error page has no React fiber' : 'the error page has a React fiber');
      await shot(p, `error-${kind}`);
      const before = asked;
      await err.getByRole('button', { name: 'Try again' }).click();
      await until('the file is asked for again', () => asked > before, 20000);
      await p.context().close();
    });
  }

  // The text of the browser is made safe by `displayText` of the app layer (rule U9), which is in the WASM module: with the module
  // gone there is no helper, so the page shows no text of the browser at all, only what it says itself (`pageFor`, `safe` null):
  // on the development server one fixed sentence about the WASM bridge, in a built page nothing more.
  await check('the error page for an editor that could not start (the WASM module does not load): its own words, none of the browser\'s, and Try again', async () => {
    const p = await newPage();
    await p.route(/\.wasm(\?.*)?$/, (route) => route.abort('failed'));
    await p.goto(page_url());
    const err = p.locator('[data-testid=error-page]');
    await err.waitFor({ state: 'visible', timeout: 20000 });
    assert.equal(await err.getAttribute('data-kind'), 'start');
    assert.match(await err.locator('h1').textContent(), /^The editor could not start$/);
    const hint = 'If the WASM bridge is not built, run `just wasm-build`, then reload.';
    const detail = err.locator('[data-testid=error-detail]');
    if (await detail.count()) assert.equal((await detail.textContent()).trim(), hint, 'the detail is the fixed development sentence, not the message of the browser');
    assert.match(await err.textContent(), /^The editor could not start\s*Nothing was changed\.\s*(If the WASM bridge is not built, run `just wasm-build`, then reload\.\s*)?Try again$/);
    assert.equal(await err.getByRole('button', { name: 'Try again' }).count(), 1);
    assert.equal(await fibre(p, '[data-testid=error-page]'), react);
    await shot(p, 'error-start');
    await p.context().close();
  });

  if (react) {
    // The React copy loads on demand. A chunk that does not load must leave the Svelte copy, never a blank page.
    await check('React chunk that does not load: the error page of an editor that could not start is drawn by Svelte', async () => {
      const p = await newPage();
      await p.route(/\.wasm(\?.*)?$/, (route) => route.abort('failed'));
      await p.route((u) => chunk.test(u.pathname), (route) => route.abort('failed'));
      await p.goto(page_url());
      const err = p.locator('[data-testid=error-page]');
      await err.waitFor({ state: 'visible', timeout: 20000 });
      assert.equal(await err.getAttribute('data-kind'), 'start');
      assert.equal(await err.getByRole('button', { name: 'Try again' }).count(), 1);
      assert.equal(await fibre(p, '[data-testid=error-page]'), false, 'the page is the Svelte copy');
      await p.context().close();
    });
  }

  await check('no uncaught error in the page that was cut, refused and made to disagree', async () => assert.deepEqual(pageErrors, []));
  await browser.close();
  const failed = results.filter((r) => !r).length;
  console.log(failed ? `${failed} of ${results.length} checks failed` : `all ${results.length} checks passed`);
  process.exit(failed ? 1 : 0);
})().catch(async (e) => {
  console.error(`feedback-e2e: ${e.message}`);
  await browser?.close().catch(() => {});
  process.exit(1);
});
