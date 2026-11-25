// Browser check of the state matrix: node packages/web-chrome/scripts/states-e2e.mjs [web-url]
// The React copy of the page is checked by the same script with MELDFRAME_UI=react (it opens /?states&react); without it the Svelte page.
// Needs the Vite dev server (`just web`, or `npx vite` in apps/web with MELDFRAME_WEB_TOKEN set to any 16 or more
// characters: this page needs no server) and Playwright from the global module path. Hand-run, not in CI.
// It Tabs through /?states in both themes and asserts, on the computed styles: the focus ring (2 px, offset 2 px,
// the selection colour of that theme), a 24 px minimum hit area, a name on every icon-only button, the disabled
// opacity, and that the drawn focus state carries the same ring as the real one.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const { chromium } = createRequire(import.meta.url)('playwright');
const web = process.argv[2] ?? 'http://localhost:5173';
const react = process.env.MELDFRAME_UI === 'react';
const browser = await chromium.launch();
const errors = [];

for (const theme of ['dark', 'light']) {
  const page = await (await browser.newContext({ viewport: { width: 1440, height: 900 }, reducedMotion: 'reduce' })).newPage();
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript((t) => localStorage.setItem('meldframe.theme', t), theme);
  await page.goto(`${web}/?states${react ? '&react' : ''}`);
  await page.waitForSelector(`section[data-theme="${theme}"] button`);

  // Computed facts about every control of this theme's matrix.
  const facts = await page.evaluate((t) => {
    const root = document.querySelector(`section[data-theme="${t}"]`);
    const sel = getComputedStyle(root).getPropertyValue('--mf-selection').trim().toLowerCase();
    const focusable = [...root.querySelectorAll('button, input')];
    return {
      sel,
      count: focusable.length,
      small: focusable.filter((e) => !e.matches('input') && (e.getBoundingClientRect().width < 24 || e.getBoundingClientRect().height < 24)).map((e) => e.outerHTML.slice(0, 80)),
      unnamed: [...root.querySelectorAll('button')].filter((b) => !b.textContent.trim() && !b.getAttribute('aria-label')?.trim()).map((b) => b.outerHTML.slice(0, 80)),
      // effective opacity: the control's own times that of its ancestors (a disabled group dims as a whole)
      disabledOpacity: [...root.querySelectorAll('button:disabled')].map((b) => { let o = 1; for (let e = b; e; e = e.parentElement) o *= Number(getComputedStyle(e).opacity); return String(o); }),
    };
  }, theme);
  assert.ok(facts.count > 20, `${theme}: only ${facts.count} controls`);
  assert.deepEqual(facts.small, [], `${theme}: a hit area under 24 px`);
  assert.deepEqual(facts.unnamed, [], `${theme}: an icon-only button without a name`);
  assert.ok(facts.disabledOpacity.length >= 4 && facts.disabledOpacity.every((o) => o === '0.5'), `${theme}: disabled opacity ${facts.disabledOpacity}`);

  // The real ring: Tab through the page and read the outline of each focused control.
  let seen = 0;
  for (let i = 0; i < 90; i++) {
    await page.keyboard.press('Tab');
    const f = await page.evaluate(() => {
      const e = document.activeElement;
      if (!e || e === document.body) return null;
      const s = getComputedStyle(e);
      const ring = e.matches('input') ? getComputedStyle(e.closest('label')) : s;
      return { id: e.outerHTML.slice(0, 60), inSection: e.closest('section')?.dataset.theme, w: ring.outlineWidth, o: ring.outlineOffset, st: ring.outlineStyle, c: ring.outlineColor };
    });
    if (!f || f.inSection !== theme) continue;
    seen++;
    const hex = (c) => '#' + c.match(/\d+/g).slice(0, 3).map((n) => Number(n).toString(16).padStart(2, '0')).join('');
    assert.equal(f.w, '2px', `${theme}: ring width on ${f.id}`);
    assert.equal(f.o, '2px', `${theme}: ring offset on ${f.id}`);
    assert.equal(f.st, 'solid', `${theme}: ring style on ${f.id}`);
    assert.equal(hex(f.c), facts.sel, `${theme}: ring colour on ${f.id}`);
  }
  assert.ok(seen >= 30, `${theme}: Tab reached only ${seen} controls`);

  // The drawn focus state has the same ring.
  const drawn = await page.evaluate((t) => [...document.querySelectorAll(`section[data-theme="${t}"] [data-preview="focus"] button`)].map((b) => getComputedStyle(b).outlineWidth), theme);
  assert.ok(drawn.length >= 5 && drawn.every((w) => w === '2px'), `${theme}: drawn focus ring ${drawn}`);
  console.log(`${theme}: ok (${facts.count} controls, ${seen} reached by Tab)`);
}
assert.deepEqual(errors, [], 'page errors');
await browser.close();
console.log(react ? 'states-e2e: ok (react)' : 'states-e2e: ok');
