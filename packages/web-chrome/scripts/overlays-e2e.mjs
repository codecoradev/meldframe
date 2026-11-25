// Browser check of the overlays of the chrome, Svelte and React against the same questions:
//   node packages/web-chrome/scripts/overlays-e2e.mjs [web-url]     (both; MELDFRAME_UI=svelte or react runs one)
// Needs the Vite dev server (`npx vite` in apps/web with MELDFRAME_WEB_TOKEN set to any 16 or more characters: these
// pages need no server) and Playwright from the global module path. Hand-run, not in CI. The pages are the overlay
// fixtures of the React pilot, which exist in development only: /?overlays=svelte (the Svelte IconButton, Menu and the tooltip
// they bring) and /?overlays=react (the React copies on Base UI), with the same test ids and rows (apps/web/src/react-pilot).
// For each it asserts what a person sees and does, as the Svelte version has it:
//   tooltip   opens on hover after the delay (400 ms in the provider), not before; opens at once on a neighbour inside the
//             skip window and waits again after it; shows the label and the shortcut (in the mono face); closes on leave and on
//             Escape; opens on keyboard focus; is gone while the menu of the same button is open
//   menu      opens by click (the popup takes the focus), Enter, Space and ArrowDown (the first row is highlighted and focused);
//             ArrowDown, ArrowUp, Home, End and a letter move between rows, wrap, and skip a disabled row; Enter or a click on a row
//             calls its handler, closes the menu and gives the focus back to the button; Escape closes and does the same; a click outside
//             closes; a disabled row does nothing; a row that opens a dialog leaves the focus in the dialog; the popup sits 8 px from
//             its button, aligned as asked, and carries data-owns-keys
// The delay is read in the page, from the pointer entering the control to the tooltip being added: 350 to 600 ms for the 400 asked.
// It also checks the real shell on /?layout (Svelte): the same tooltip and a menu of the layout, so the fixture is not the only reference.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const { chromium } = createRequire(import.meta.url)('playwright');
const web = process.argv[2] ?? 'http://localhost:5173';
const only = process.env.MELDFRAME_UI;
const uis = only ? [only] : ['svelte', 'react'];
const browser = await chromium.launch();
const errors = [];

async function open(url, tag) {
  const page = await (await browser.newContext({ viewport: { width: 1000, height: 700 }, reducedMotion: 'reduce' })).newPage();
  page.on('pageerror', (e) => errors.push(`${tag}: ${e}`));
  await page.goto(url);
  await page.evaluate(() => document.fonts.ready);
  return page;
}
const tip = (page) => page.locator('[role=tooltip]');
const menu = (page) => page.locator('[role=menu]');
const log = (page) => page.locator('[data-testid=fx-log]').innerText();
/** What has the focus: a test id, a role, or the tag. */
const active = (page) => page.evaluate(() => { const e = document.activeElement; return e === document.body ? 'body' : e.getAttribute('data-testid') || e.getAttribute('role') || e.tagName; });
/** The rows by short name: `*` highlighted, `(f)` holds the focus. */
const rows = (page) => page.evaluate(() => [...document.querySelectorAll('[role=menuitem]')].map((e) => e.dataset.testid.slice(3) + (e.hasAttribute('data-highlighted') ? '*' : '') + (document.activeElement === e ? '(f)' : '')).join(' '));
const focused = (page) => page.evaluate(() => document.activeElement?.dataset.testid);
const moveTo = async (page, id) => { const b = await page.locator(`[data-testid=${id}]`).boundingBox(); await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2); };
/**
 * Moves to a control and returns how long the tooltip took to appear in the page, from the pointer entering the control to the
 * element being added (read in the page: the round trips of the test do not count), or null when it did not appear within `within` ms.
 */
async function hoverDelay(page, id, within = 2000) {
  await page.evaluate((testid) => {
    const mark = (window.__tip = {});
    document.querySelector(`[data-testid=${testid}]`).addEventListener('pointerenter', () => { mark.enter = performance.now(); }, { once: true });
    new MutationObserver((_, o) => { if (document.querySelector('[role=tooltip]')) { mark.shown = performance.now(); o.disconnect(); } }).observe(document.body, { childList: true, subtree: true });
  }, id);
  await moveTo(page, id);
  try {
    await tip(page).waitFor({ timeout: within });
  } catch {
    return null;
  }
  const { enter, shown } = await page.evaluate(() => window.__tip);
  return Math.round(shown - enter);
}
const shut = async (page) => { await page.mouse.move(5, 650); await tip(page).waitFor({ state: 'detached', timeout: 2000 }); };
const openMenu = async (page, id = 'fx-menu') => { await page.click(`[data-testid=${id}]`); await menu(page).waitFor(); await page.waitForTimeout(150); };
const closed = (page) => menu(page).waitFor({ state: 'detached' }).then(() => page.waitForTimeout(150));

async function tooltips(ui) {
  const page = await open(`${web}/?overlays=${ui}`, ui);
  assert.equal(await page.getAttribute('[data-testid=fx-icon]', 'aria-label'), 'Layers panel', `${ui}: the trigger keeps its own name`);
  // delay: not open before 300 ms, open within 2 s; the label and the shortcut
  await moveTo(page, 'fx-icon');
  await page.waitForTimeout(200);
  assert.equal(await tip(page).count(), 0, `${ui}: the tooltip opened before the delay`);
  await tip(page).waitFor({ timeout: 2000 });
  assert.match(await tip(page).innerText(), /Layers panel\s*Alt\+1/, `${ui}: tooltip text`);
  assert.match(await tip(page).locator('span').first().evaluate((e) => getComputedStyle(e).fontFamily), /mono/i, `${ui}: shortcut in the mono face`);
  const below = await page.evaluate(() => ({ t: document.querySelector('[role=tooltip]').getBoundingClientRect().top, b: document.querySelector('[data-testid=fx-icon]').getBoundingClientRect().bottom }));
  assert.ok(Math.abs(below.t - below.b - 8) < 1, `${ui}: the tooltip sits 8 px below its button (${JSON.stringify(below)})`);
  // the skip window: a neighbour opens at once; the text changes
  const t = Date.now();
  await moveTo(page, 'fx-icon-2');
  await page.waitForFunction(() => document.querySelector('[role=tooltip]')?.textContent.includes('Inspector panel'), null, { timeout: 2000 });
  assert.ok(Date.now() - t < 300, `${ui}: a neighbour inside the skip window took ${Date.now() - t} ms`);
  // leaving closes it; after the skip window the delay applies again
  await shut(page);
  await page.waitForTimeout(600);
  await moveTo(page, 'fx-icon-2');
  await page.waitForTimeout(200);
  assert.equal(await tip(page).count(), 0, `${ui}: after the skip window the tooltip waited again`);
  const again = await hoverDelay(page, 'fx-icon', 2000);
  assert.ok(again !== null && again >= 350 && again < 600, `${ui}: the tooltip took ${again} ms after a pause`);
  await shut(page);
  // the delay itself: both fixtures take about the same
  await page.waitForTimeout(600);
  const first = await hoverDelay(page, 'fx-icon', 3000);
  assert.ok(first !== null && first >= 350 && first < 600, `${ui}: the first tooltip took ${first} ms (400 ms asked)`);
  await shut(page);
  // keyboard focus: Tab onto the button opens it, Escape closes it and the focus stays
  await page.mouse.move(5, 650);
  await page.evaluate(() => document.activeElement?.blur());
  await page.keyboard.press('Tab');
  assert.equal(await focused(page), 'fx-icon', `${ui}: the first Tab stop`);
  await tip(page).waitFor({ timeout: 2000 });
  assert.match(await tip(page).innerText(), /Layers panel/, `${ui}: tooltip on keyboard focus`);
  await page.keyboard.press('Escape');
  await tip(page).waitFor({ state: 'detached', timeout: 2000 });
  assert.equal(await focused(page), 'fx-icon', `${ui}: Escape keeps the focus on the button`);
  // a tooltip that is open when the menu of the same button opens closes
  await shut(page);
  await page.waitForTimeout(400);
  assert.ok((await hoverDelay(page, 'fx-menu')) !== null, `${ui}: the menu button has a tooltip`);
  await page.mouse.down();
  await page.mouse.up();
  await menu(page).waitFor();
  await page.waitForTimeout(300);
  assert.equal(await tip(page).count(), 0, `${ui}: a tooltip stays open over the opened menu`);
  await page.context().close();
  return { first, again };
}

async function menus(ui) {
  const fresh = () => open(`${web}/?overlays=${ui}`, ui);
  let page = await fresh();
  // opened by a click: the popup takes the focus and no row is chosen yet
  await openMenu(page);
  assert.equal(await menu(page).count(), 1);
  assert.equal(await page.locator('[role=menu][data-owns-keys]').count(), 1, `${ui}: the menu carries the key marker`);
  assert.equal(await active(page), 'menu', `${ui}: a click puts the focus on the popup`);
  assert.equal(await rows(page), 'alpha bravo charlie dialog', `${ui}: nothing chosen after a click`);
  assert.equal(await page.getAttribute('[data-testid=fx-menu]', 'aria-haspopup'), 'menu');
  assert.equal(await page.getAttribute('[data-testid=fx-menu]', 'aria-expanded'), 'true');
  assert.equal(await page.getAttribute('[data-testid=fx-bravo]', 'aria-disabled'), 'true', `${ui}: the disabled row says so`);
  // placement: 8 px below the button, aligned with its start
  const at = await page.evaluate(() => { const m = document.querySelector('[role=menu]').getBoundingClientRect(); const b = document.querySelector('[data-testid=fx-menu]').getBoundingClientRect(); return { top: m.top - b.bottom, left: m.left - b.left, w: m.width }; });
  assert.ok(Math.abs(at.top - 8) < 0.5 && Math.abs(at.left) < 0.5 && at.w === 192, `${ui}: placement ${JSON.stringify(at)}`);
  // arrows: first row, skip the disabled one, wrap, Home, End, a letter
  const walk = async (key, want) => { await page.keyboard.press(key); await page.waitForTimeout(50); assert.equal(await rows(page), want, `${ui}: after ${key}`); };
  await walk('ArrowDown', 'alpha*(f) bravo charlie dialog');
  await walk('ArrowDown', 'alpha bravo charlie*(f) dialog');
  await walk('ArrowDown', 'alpha bravo charlie dialog*(f)');
  await walk('ArrowDown', 'alpha*(f) bravo charlie dialog');
  await walk('ArrowUp', 'alpha bravo charlie dialog*(f)');
  await walk('ArrowUp', 'alpha bravo charlie*(f) dialog');
  await walk('End', 'alpha bravo charlie dialog*(f)');
  await walk('Home', 'alpha*(f) bravo charlie dialog');
  await walk('c', 'alpha bravo charlie*(f) dialog');
  // Escape closes and gives the focus back to the button
  await page.keyboard.press('Escape');
  await closed(page);
  assert.equal(await active(page), 'fx-menu', `${ui}: Escape gives the focus back to the button`);
  // Enter, Space and ArrowDown on the button open it with the first row highlighted and focused
  for (const key of ['Enter', 'Space', 'ArrowDown']) {
    await page.keyboard.press(key);
    await menu(page).waitFor();
    await page.waitForTimeout(150);
    assert.equal(await rows(page), 'alpha*(f) bravo charlie dialog', `${ui}: ${key} on the button`);
    await page.keyboard.press('Escape');
    await closed(page);
    assert.equal(await active(page), 'fx-menu', `${ui}: the focus after ${key} and Escape`);
  }
  // Enter on a row calls its handler once, closes and gives the focus back
  await page.keyboard.press('Enter');
  await menu(page).waitFor();
  await page.waitForTimeout(150);
  await page.keyboard.press('ArrowDown'); // from the first row, past the disabled one, to the third
  await page.keyboard.press('Enter');
  await closed(page);
  assert.equal(await log(page), 'charlie', `${ui}: Enter on the row (the disabled row between is skipped)`);
  assert.equal(await active(page), 'fx-menu', `${ui}: the focus after Enter on a row`);
  // a click on a row does the same; a click on the disabled row does nothing and the menu stays
  await openMenu(page);
  await page.click('[data-testid=fx-bravo]', { force: true }); // Playwright waits for an enabled element: the click is made anyway
  await page.waitForTimeout(300);
  assert.equal(await menu(page).count(), 1, `${ui}: a click on the disabled row closed the menu`);
  assert.equal(await log(page), 'charlie', `${ui}: a click on the disabled row called a handler`);
  await page.hover('[data-testid=fx-alpha]');
  await page.waitForTimeout(100);
  assert.match(await rows(page), /alpha\*\(f\)/, `${ui}: hovering a row highlights and focuses it`);
  await page.click('[data-testid=fx-alpha]');
  await closed(page);
  assert.equal(await log(page), 'charlie,alpha', `${ui}: a click on the row`);
  assert.equal(await active(page), 'fx-menu', `${ui}: the focus after a click on a row`);
  // a click outside closes it
  await openMenu(page);
  await page.mouse.click(600, 600);
  await closed(page);
  assert.equal(await menu(page).count(), 0, `${ui}: a click outside left the menu open`);
  await page.context().close();

  // a row that opens a dialog: the focus stays where the dialog took it
  page = await fresh();
  await openMenu(page);
  await page.click('[data-testid=fx-dialog]');
  await closed(page);
  await page.waitForTimeout(300);
  assert.equal(await active(page), 'fx-dialog-input', `${ui}: the menu took the focus back from the dialog`);
  await page.context().close();

  // the text trigger (the zoom readout): its text, its name, popup above and aligned to the end
  page = await fresh();
  assert.equal(await page.innerText('[data-testid=fx-text-menu]'), '100%');
  assert.equal(await page.getAttribute('[data-testid=fx-text-menu]', 'aria-label'), 'Zoom 100%');
  await openMenu(page, 'fx-text-menu');
  const top = await page.evaluate(() => { const m = document.querySelector('[role=menu]').getBoundingClientRect(); const b = document.querySelector('[data-testid=fx-text-menu]').getBoundingClientRect(); return { gap: b.top - m.bottom, right: b.right - m.right }; });
  assert.ok(Math.abs(top.gap - 8) < 0.5 && Math.abs(top.right) < 0.5, `${ui}: text menu placement ${JSON.stringify(top)}`);
  await page.keyboard.press('Escape');
  await closed(page);
  assert.equal(await active(page), 'fx-text-menu', `${ui}: the focus goes back to the text button`);
  await page.context().close();
}

for (const ui of uis) {
  const t = await tooltips(ui);
  await menus(ui);
  console.log(`${ui}: ok (first tooltip ${t.first} ms, after a pause ${t.again} ms)`);
}

// The real shell, Svelte: a tool's tooltip and the zoom menu of /?layout.
if (!only || only === 'svelte') {
  const page = await open(`${web}/?layout`, 'layout');
  await page.waitForSelector('[data-testid=toolbar]');
  const ms = await hoverDelay(page, 'tool-frame', 3000);
  assert.ok(ms !== null && ms >= 350 && ms < 600, `layout: the tooltip of a tool took ${ms} ms`);
  assert.match(await tip(page).innerText(), /Frame\s*F/);
  await page.click('[data-testid=zoom]');
  await menu(page).waitFor();
  await page.keyboard.press('Escape');
  await closed(page);
  assert.equal(await active(page), 'zoom', 'layout: Escape gives the focus back to the zoom readout');
  await page.context().close();
  console.log('layout (svelte reference): ok');
}

assert.deepEqual(errors, [], 'page errors');
await browser.close();
console.log('overlays-e2e: ok');
