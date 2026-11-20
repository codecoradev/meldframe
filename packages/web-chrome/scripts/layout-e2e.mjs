// Browser check of the layout: node packages/web-chrome/scripts/layout-e2e.mjs [web-url] [shots-dir]
// Needs the Vite dev server (`npx vite` in apps/web with MELDFRAME_WEB_TOKEN set to any 16 or more characters: /?layout
// needs no server) and Playwright from the global module path. Hand-run, not in CI. At 1440x900 and 1280x800, in both
// themes, it asserts on the computed boxes: the Layers card (inset 12, 260 wide, radius 12, 1 px border, header 52), the
// one right card (inset 12, 288, header 52, tabs Design and Activity), the tool bar (36 px buttons, padding 8, gap 4,
// bottom 20) and that no two pieces of chrome overlap; then Option+1, Option+2 and ? . With a shots-dir it saves screenshots (DPR 2).
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const { chromium } = createRequire(import.meta.url)('playwright');
const web = process.argv[2] ?? 'http://localhost:5173';
const shots = process.argv[3];
const browser = await chromium.launch();
const errors = [];
const box = (page, sel) => page.locator(sel).first().evaluate((e) => { const r = e.getBoundingClientRect(); const s = getComputedStyle(e); return { x: r.x, y: r.y, w: r.width, h: r.height, r: r.right, b: r.bottom, radius: s.borderTopLeftRadius, border: s.borderTopWidth, pad: s.paddingTop, gap: s.columnGap }; });
const overlap = (a, b) => a.x < b.r && b.x < a.r && a.y < b.b && b.y < a.b;

for (const [vw, vh] of [[1440, 900], [1280, 800]]) {
  for (const theme of ['dark', 'light']) {
    const page = await (await browser.newContext({ viewport: { width: vw, height: vh }, deviceScaleFactor: 2, reducedMotion: 'reduce' })).newPage();
    page.on('pageerror', (e) => errors.push(String(e)));
    await page.addInitScript((t) => localStorage.setItem('meldframe.theme', t), theme);
    await page.goto(`${web}/?layout`);
    await page.waitForSelector('[data-testid=toolbar]');
    await page.evaluate(() => document.fonts.ready);
    const at = `${vw}x${vh} ${theme}`;
    const left = await box(page, '[data-testid=layers-card]');
    const right = await box(page, '[data-testid=right-card]');
    const bar = await box(page, '[data-testid=toolbar]');
    assert.deepEqual([left.x, left.y, left.w, vh - left.b], [12, 12, 260, 12], `${at}: left card inset and width`);
    assert.deepEqual([vw - right.r, right.y, right.w, vh - right.b], [12, 12, 288, 12], `${at}: right card inset and width`);
    for (const [name, c] of [['left', left], ['right', right]]) {
      assert.equal(c.radius, '12px', `${at}: ${name} card radius`);
      assert.equal(c.border, '1px', `${at}: ${name} card border`);
    }
    assert.equal((await box(page, '[data-testid=layers-card] > header')).h, 52, `${at}: left header 52`);
    assert.equal((await box(page, '[data-testid=right-card] > header')).h, 52, `${at}: right header 52`);
    assert.deepEqual(await page.locator('[data-testid=right-card] [role=tab]').allTextContents(), ['Design', 'Activity'], `${at}: tabs`);
    assert.equal(await page.locator('text=/^Share$/').count(), 0, `${at}: no Share`);
    // the tool bar
    assert.equal(bar.pad, '8px', `${at}: bar padding`);
    assert.equal(bar.gap, '4px', `${at}: bar gap`);
    assert.equal(vh - bar.b, 20, `${at}: bar 20 from the bottom`);
    assert.ok(Math.abs(bar.x + bar.w / 2 - vw / 2) < 1, `${at}: bar centred`);
    const sizes = await page.locator('[data-testid=toolbar] button').evaluateAll((bs) => bs.map((b) => { const r = b.getBoundingClientRect(); return [r.width, r.height]; }));
    assert.ok(sizes.length >= 9 && sizes.every(([w, h]) => w === 36 && h === 36), `${at}: 36 px buttons ${JSON.stringify(sizes)}`);
    // nothing overlaps
    const parts = { left, right, bar, status: await box(page, '[data-testid=connection]'), zoom: await box(page, '[data-testid=zoom]'), help: await box(page, 'button[aria-label=Shortcuts]') };
    const names = Object.keys(parts);
    for (let i = 0; i < names.length; i++) for (let j = i + 1; j < names.length; j++) assert.ok(!overlap(parts[names[i]], parts[names[j]]), `${at}: ${names[i]} overlaps ${names[j]}`);
    assert.ok(bar.x - left.r >= 16 && right.x - bar.r >= 16, `${at}: 16 px clear of the panels`);
    // tooltip with the shortcut in mono
    await page.hover('[data-testid=tool-frame]');
    const tip = page.locator('[role=tooltip]');
    await tip.waitFor();
    assert.match(await tip.innerText(), /Frame\s*F/, `${at}: tooltip text`);
    assert.match(await tip.locator('span').first().evaluate((e) => getComputedStyle(e).fontFamily), /mono/i, `${at}: shortcut in mono`);
    // two triggers on one button: a tooltip that is open when the menu opens closes (the tooltip's pointerdown is kept)
    await page.hover('[data-testid=file-menu]');
    await tip.waitFor();
    await page.click('[data-testid=file-menu]');
    await page.locator('[role=menu]').waitFor();
    await page.waitForTimeout(250);
    assert.equal(await page.locator('[role=tooltip]').count(), 0, `${at}: a tooltip stays open over the opened menu`);
    const item = await box(page, '[role=menu] [role=menuitem]');
    assert.ok(item.y > 0 && item.b < vh && item.x >= 0 && item.r <= vw, `${at}: the menu is not placed next to its button (${JSON.stringify(item)})`);
    assert.ok(await page.locator('[role=menu][data-owns-keys]').count() === 1, `${at}: the menu carries the key marker`);
    await page.keyboard.press('Escape');
    await page.locator('[role=menu]').waitFor({ state: 'detached' });
    // a disabled tab is not reached by the arrow keys: the right card keeps its width and the Design panel
    await page.focus('#right-tab-design');
    await page.keyboard.press('ArrowRight');
    await page.keyboard.press('End');
    assert.equal(await page.getAttribute('#right-tab-design', 'aria-selected'), 'true', `${at}: the arrow moved to the disabled tab`);
    assert.equal(await page.getAttribute('#right-tab-activity', 'aria-selected'), 'false', `${at}: Activity selected`);
    assert.equal((await box(page, '[data-testid=right-card]')).w, 288, `${at}: the right card changed width`);
    // hit areas of the controls inside the cards: 24 px at least
    const small = await page.locator('[data-owns-keys] button, [data-owns-keys] [role=tab]').evaluateAll((bs) => bs.filter((b) => { const r = b.getBoundingClientRect(); return r.width < 24 || r.height < 24; }).map((b) => b.outerHTML.slice(0, 80)));
    assert.deepEqual(small, [], `${at}: a hit area under 24 px`);
    await page.mouse.move(5, 5);
    if (shots) await page.screenshot({ path: `${shots}/layout-${vw}x${vh}-${theme}.png` });
    if (theme === 'dark') {
      // keys: Option+1 and Option+2 fold the panels into the two 52 px cards; ? opens the hint, Escape closes it
      await page.keyboard.press('Alt+Digit1');
      await page.waitForSelector('[data-testid=top-left-card]');
      const tl = await box(page, '[data-testid=top-left-card]');
      assert.deepEqual([tl.x, tl.y, tl.h], [16, 16, 52], `${at}: top-left card`);
      assert.equal(await page.locator('[data-testid=layers-card]').count(), 0);
      await page.keyboard.press('Alt+Digit2');
      await page.waitForSelector('[data-testid=top-right-card]');
      const tr = await box(page, '[data-testid=top-right-card]');
      assert.deepEqual([vw - tr.r, tr.y, tr.h], [16, 16, 52], `${at}: top-right card`);
      assert.equal(await page.locator('[data-testid=right-card]').count(), 0);
      assert.equal(await page.getAttribute('[data-testid=toggle-layers]', 'aria-pressed'), 'false');
      await page.keyboard.press('Alt+Digit1');
      await page.waitForSelector('[data-testid=layers-card]');
      await page.keyboard.press('Shift+Slash');
      await page.waitForSelector('[data-testid=shortcut-hint]');
      await page.keyboard.press('Escape');
      assert.equal(await page.locator('[data-testid=shortcut-hint]').count(), 0);
      // focus follows a toggle: the header button of a card is swapped for the one of its folded form, and back
      const focusIn = (sel) => page.evaluate((q) => document.activeElement === document.querySelector(q), sel);
      await page.focus('[data-testid=layers-card] [data-focus=layers]');
      await page.keyboard.press('Enter');
      await page.waitForSelector('[data-testid=top-left-card]');
      assert.ok(await focusIn('[data-testid=top-left-card] [data-focus=layers]'), `${at}: focus lost when the layers card folds`);
      await page.keyboard.press('Enter');
      await page.waitForSelector('[data-testid=layers-card]');
      assert.ok(await focusIn('[data-testid=layers-card] [data-focus=layers]'), `${at}: focus lost when the layers card opens`);
      await page.keyboard.press('Alt+Digit2');
      await page.waitForSelector('[data-testid=right-card]');
      await page.focus('[data-testid=right-card] [data-focus=inspector]');
      await page.keyboard.press('Enter');
      await page.waitForSelector('[data-testid=top-right-card]');
      assert.ok(await focusIn('[data-testid=top-right-card] [data-focus=inspector]'), `${at}: focus lost when the right card folds`);
      await page.keyboard.press('Enter');
      await page.waitForSelector('[data-testid=right-card]');
      assert.ok(await focusIn('[data-testid=right-card] [data-focus=inspector]'), `${at}: focus lost when the right card opens`);
      // the hint takes the focus, gives it back on Escape, and an Escape with the focus elsewhere does not close it
      await page.focus('button[aria-label=Shortcuts]');
      await page.keyboard.press('Enter');
      await page.waitForSelector('[data-testid=shortcut-hint]');
      assert.ok(await page.evaluate(() => document.querySelector('[data-testid=shortcut-hint]').contains(document.activeElement)), `${at}: the hint did not take the focus`);
      await page.keyboard.press('Escape');
      assert.equal(await page.locator('[data-testid=shortcut-hint]').count(), 0);
      assert.ok(await focusIn('button[aria-label=Shortcuts]'), `${at}: the focus did not go back to the help button`);
      await page.keyboard.press('Shift+Slash');
      await page.waitForSelector('[data-testid=shortcut-hint]');
      await page.focus('[data-testid=toggle-layers]');
      await page.keyboard.press('Escape');
      assert.equal(await page.locator('[data-testid=shortcut-hint]').count(), 1, `${at}: an Escape elsewhere closed the hint`);
      await page.focus('[data-testid=shortcut-hint] button');
      await page.keyboard.press('Escape');

      assert.equal(await page.locator('[data-testid=shortcut-hint]').count(), 0);
      if (shots) { await page.keyboard.press('Alt+Digit2'); await page.screenshot({ path: `${shots}/layout-closed-${vw}x${vh}-${theme}.png` }); }
    }
    console.log(`${at}: ok`);
    await page.context().close();
  }
}
assert.deepEqual(errors, [], 'page errors');
await browser.close();
console.log('layout-e2e: ok');
