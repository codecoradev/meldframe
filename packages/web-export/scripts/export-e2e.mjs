// Browser check of the Export section and dialog against a real server: node packages/web-export/scripts/export-e2e.mjs [web-url] [server-url]
// Needs `just dev` (server on 8080) and `just web` (Vite on 5173) running, the WASM bridge built (`just wasm-build`) and
// Playwright from the global module path. It checks the PNG at 0.5x, 1x, 2x, 3x and 4x (file name and pixel size of each saved file) and a scale over the 4096 x 4096 area limit (1440 x 900 at 4x), which is refused before saving. It adds a frame, a shape and a text with a shadow (which the export cannot carry)
// to the file `MELDFRAME_WEB_FILE` (default `file_dev`) under names of its own. Hand-run, not in CI: it needs a browser and a server.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';

const { chromium } = createRequire(import.meta.url)('playwright');
const web = process.argv[2] ?? 'http://localhost:5173';
const server = process.argv[3] ?? 'http://127.0.0.1:8080';
const root = new URL('../../../', import.meta.url).pathname;
const token = readFileSync(`${root}.dev-token`, 'utf8').trim();
const file = process.env.MELDFRAME_WEB_FILE ?? 'file_dev';
const run = Math.random().toString(36).slice(2, 8);
const frame = `Landing ${run}`;
let n = 0;

async function post(command) {
  const res = await fetch(`${server}/v1/files/${file}/tools/apply_commands`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', authorization: `Bearer ${token}` },
    body: JSON.stringify({ ops: [{ key: `export-e2e-${run}-${++n}`, command, deps: [] }] }),
  });
  assert.equal(res.status, 200, `op refused: ${res.status} ${await res.text()}`);
}
const pngSize = (b) => [b.readUInt32BE(16), b.readUInt32BE(20)];
const results = [];
async function check(name, fn) {
  try { await fn(); results.push(true); console.log('ok  ', name); } catch (e) { results.push(false); console.log('FAIL', name, '\n    ', String(e.message ?? e).split('\n')[0]); }
}

const f = `f_${run}`;
await post({ type: 'addNode', node: { id: f, name: frame, type: 'frame', parentId: null, x: 0, y: 0, w: 600, h: 400, fill: '#e6ece8' } });
await post({ type: 'addNode', node: { id: `r_${run}`, name: `Card ${run}`, type: 'rect', parentId: f, x: 20, y: 80, w: 200, h: 120, fill: '#9aa8a0' } });
await post({ type: 'addNode', node: { id: `t_${run}`, name: `Hero Title ${run}`, type: 'text', parentId: f, x: 20, y: 20, text: 'Design with your agent', fontSize: 24, shadows: [{ x: 0, y: 2, blur: 4, spread: 0, color: '#00000040' }] } });

const browser = await chromium.launch({ args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 }, acceptDownloads: true, permissions: ['clipboard-read', 'clipboard-write'] });
const page = await ctx.newPage();
const pageErrors = [];
page.on('pageerror', (e) => pageErrors.push(e.message));
await page.goto(web);
await page.waitForSelector(`text=${frame}`);
const layer = page.locator('[data-testid=layers-card]').getByText(frame, { exact: true });
const dialog = page.locator('[data-testid=export-dialog]');

await check('the Export section appears for a selected frame only', async () => {
  assert.equal(await page.locator('[data-testid=export-section]').count(), 0, 'nothing selected');
  await page.locator('[data-testid=layers-card]').getByText(`Card ${run}`, { exact: true }).click();
  assert.equal(await page.locator('[data-testid=export-section]').count(), 0, 'a shape is selected');
  await layer.click();
  await page.waitForSelector('[data-testid=export-section]');
  assert.match(await page.locator('[data-testid=export-section-plan]').innerText(), new RegExp(`${frame}.png · 600 × 400 px`));
});
const scaleRadio = (scope, text) => page.locator(`${scope} [role=radio]`, { hasText: new RegExp(`^${text}$`) });
const savedPng = async (button) => {
  const [dl] = await Promise.all([page.waitForEvent('download'), page.click(button)]);
  return { name: dl.suggestedFilename(), size: pngSize(readFileSync(await dl.path())) };
};
await check('the Export section offers 0.5x, 1x, 2x, 3x and 4x', async () => {
  assert.deepEqual(await page.locator('[data-testid=export-section] [role=radio]').allInnerTexts(), ['0.5x', '1x', '2x', '3x', '4x']);
});
await check('Export as PNG saves the frame at 2x, 1200 x 800', async () => {
  await scaleRadio('[data-testid=export-section]', '2x').click();
  const got = await savedPng('[data-testid=export-section-png]');
  assert.equal(got.name, `${frame}@2x.png`);
  assert.deepEqual(got.size, [1200, 800]);
});
await check('0.5x saves 300 x 200 as @0.5x, 3x saves 1800 x 1200 as @3x, 4x saves 2400 x 1600 as @4x', async () => {
  for (const [text, name, size] of [['0.5x', `${frame}@0.5x.png`, [300, 200]], ['3x', `${frame}@3x.png`, [1800, 1200]], ['4x', `${frame}@4x.png`, [2400, 1600]]]) {
    await scaleRadio('[data-testid=export-section]', text).click();
    assert.match(await page.locator('[data-testid=export-section-plan]').innerText(), new RegExp(`${name.replace('.', '\\.')} · ${size[0]} × ${size[1]} px`));
    const got = await savedPng('[data-testid=export-section-png]');
    assert.equal(got.name, name);
    assert.deepEqual(got.size, size, text);
  }
});
await check('a scale over the area limit is refused before saving, with the largest scale that fits (1440 x 900 at 4x)', async () => {
  await post({ type: 'updateNode', id: f, patch: { w: 1440, h: 900 } });
  await page.waitForFunction(() => document.querySelector('[data-testid=export-section-plan]')?.textContent?.includes('5760 × 3600'));
  const text = await page.locator('[data-testid=export-section-plan]').innerText();
  assert.match(text, /At 4x the image would be 5760 × 3600 px, more than the limit of 4096 × 4096 px in area\./);
  assert.match(text, /The largest scale that fits is 3x \(4320 × 2700 px\)\./);
  assert.equal(await page.locator('[data-testid=export-section-png]').isDisabled(), true);
  await scaleRadio('[data-testid=export-section]', '3x').click();
  await page.waitForFunction(() => document.querySelector('[data-testid=export-section-png]')?.hasAttribute('disabled') === false);
  const got = await savedPng('[data-testid=export-section-png]');
  assert.equal(got.name, `${frame}@3x.png`);
  assert.deepEqual(got.size, [4320, 2700]);
  await post({ type: 'updateNode', id: f, patch: { w: 600, h: 400 } });
  await page.waitForFunction(() => document.querySelector('[data-testid=export-section-plan]')?.textContent?.includes('1800 × 1200')); // back to 600 x 400, at 3x
});
await check('the dialog opens on the frame, with the HTML, the report and no focus on a tooltip', async () => {
  await page.click('[data-testid=export-section-html]');
  await dialog.waitFor();
  assert.match(await dialog.locator('[data-dialog-title]').innerText(), new RegExp(`Export ${frame}`));
  assert.ok((await dialog.locator('[data-testid=export-code]').innerText()).includes('<!doctype html>'));
  const unsupported = await dialog.locator('[data-testid=export-unsupported]').innerText();
  assert.ok(unsupported.includes(`Hero Title ${run}`) && unsupported.includes('on_text'), unsupported);
  assert.equal(await page.evaluate(() => !!document.activeElement?.closest('[role=dialog]')), true, 'focus is in the dialog');
});
await check('Copy puts the whole document on the clipboard, Download saves it under the frame name', async () => {
  await dialog.locator('[data-testid=export-copy]').click();
  await page.waitForFunction(() => document.querySelector('[data-testid=export-said]')?.textContent?.includes('copied'));
  const clip = await page.evaluate(() => navigator.clipboard.readText());
  const [dl] = await Promise.all([page.waitForEvent('download'), dialog.locator('[data-testid=export-download]').click()]);
  assert.equal(dl.suggestedFilename(), `${frame}.html`);
  assert.equal(readFileSync(await dl.path(), 'utf8'), clip);
  assert.ok(clip.includes(`data-node="${f}"`));
});
await check('plain CSS has its styles inline and no second file; Tailwind has the stylesheet the HTML links', async () => {
  await dialog.locator('[data-testid=export-file] [role=radio]', { hasText: 'meldframe.css' }).click();
  const [dl] = await Promise.all([page.waitForEvent('download'), dialog.locator('[data-testid=export-download]').click()]);
  assert.equal(dl.suggestedFilename(), 'meldframe.css');
  await dialog.locator('[data-testid=export-format] [role=radio]', { hasText: 'Plain CSS' }).click();
  await page.waitForFunction(() => document.querySelector('[data-testid=export-code]')?.textContent?.includes('<style'));
  assert.equal(await dialog.locator('[data-testid=export-file]').count(), 0);
});
await check('the PNG tab shows the file and size and saves it', async () => {
  await dialog.locator('#export-tab-png').click();
  assert.deepEqual(await dialog.locator('[data-testid=export-scale] [role=radio]').allInnerTexts(), ['0.5x', '1x', '2x', '3x', '4x']);
  await scaleRadio('[data-testid=export-dialog] [data-testid=export-scale]', '1x').click();
  assert.match(await dialog.locator('[data-testid=export-png-plan]').innerText(), /600 × 400 px/);
  const [dl] = await Promise.all([page.waitForEvent('download'), dialog.locator('[data-testid=export-png]').click()]);
  assert.deepEqual(pngSize(readFileSync(await dl.path())), [600, 400]);
});
await check('a dialog opened again starts on the HTML tab and says nothing from the last one', async () => {
  await page.keyboard.press('Escape');
  await dialog.waitFor({ state: 'detached' });
  await page.click('[data-testid=export-section-html]');
  await dialog.waitFor();
  assert.equal(await dialog.locator('#export-tab-html').getAttribute('aria-selected'), 'true');
  assert.equal((await dialog.locator('[data-testid=export-said]').innerText()).trim(), '');
});
await check('a change by an agent shows in the open dialog, and Escape closes it', async () => {
  await post({ type: 'updateNode', id: f, patch: { fill: '#12ab34' } });
  await page.waitForFunction(() => document.querySelector('[data-testid=export-code]')?.textContent?.includes('#12ab34') || document.querySelector('[data-testid=export-code]')?.textContent?.toLowerCase().includes('12ab34'));
  await page.keyboard.press('Escape');
  await dialog.waitFor({ state: 'detached' });
});
await check('the File menu opens it too, the focus goes into the dialog, and back to the menu button when it closes', async () => {
  await page.click('[data-testid=file-menu]');
  await page.click('[data-testid=export]');
  await dialog.waitFor();
  await page.waitForTimeout(500);
  assert.equal(await page.evaluate(() => !!document.activeElement?.closest('[role=dialog]')), true);
  await dialog.getByRole('button', { name: 'Close', exact: true }).last().click();
  await dialog.waitFor({ state: 'detached' });
  await page.waitForTimeout(300);
  assert.equal(await page.evaluate(() => document.activeElement?.getAttribute('aria-label')), 'File menu', 'the focus is back on the menu button');
});
await check('the page threw no error', async () => assert.deepEqual(pageErrors, []));

await browser.close();
const failed = results.filter((r) => !r).length;
console.log(failed ? `${failed} of ${results.length} failed` : `all ${results.length} passed`);
process.exit(failed ? 1 : 0);
