// Browser check of the frame menu and the command palette on the layout page:
//   node packages/web-shell/scripts/palette-e2e.mjs [web-url] [shots-dir] [editor-url]
// Needs the Vite dev server (`npx vite` in apps/web with MELDFRAME_WEB_TOKEN set to any 16 or more characters: /?layout
// needs no server) and Playwright from the global module path. Hand-run, not in CI. On /?layout the New frame menu and
// the palette make boxes of the preset's size, so the sizes are read from the page. With an editor-url (the editor
// page of Vite with a development server behind it, `just dev` and `just web`, or a server in memory mode) it also checks
// the real editor: the Export entry and its focus, and Ctrl+K over the Export dialog, in a field and in the text editor.
// Not covered: a Mac (Command+K and the key labels are unit tests only), a screen reader, a touch screen.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const { chromium } = createRequire(import.meta.url)('playwright');
const web = process.argv[2] ?? 'http://localhost:5173';
const shots = process.argv[3];
const editor = process.argv[4];
const browser = await chromium.launch();
const errors = [];
const sizes = { a4: [595, 842], mobile: [390, 844], desktopHd: [1920, 1080], letter: [612, 792] };

for (const theme of ['dark', 'light']) {
  const page = await (await browser.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2, reducedMotion: 'reduce' })).newPage();
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript((t) => localStorage.setItem('meldframe.theme', t), theme);
  await page.goto(`${web}/?layout`);
  await page.waitForSelector('[data-testid=toolbar]');
  await page.evaluate(() => document.fonts.ready);
  const at = theme;
  const palette = page.locator('[data-testid=palette]');
  const options = page.locator('[data-testid=palette-option]');
  const active = () => page.evaluate(() => document.activeElement?.getAttribute('data-testid') ?? document.activeElement?.getAttribute('aria-label') ?? document.activeElement?.tagName);
  const tool = () => page.getAttribute('[data-testid=tool]', 'data-tool');
  const frames = () => page.locator('[data-testid=demo-frame]').evaluateAll((es) => es.map((e) => [e.getAttribute('data-preset'), e.getBoundingClientRect().width, e.getBoundingClientRect().height]));
  const open = async () => { await page.keyboard.press('Control+k'); await palette.waitFor(); await page.waitForFunction(() => document.activeElement?.getAttribute('data-testid') === 'palette-input'); };
  const shut = async () => { await page.keyboard.press('Escape'); await palette.waitFor({ state: 'detached' }); };

  // the frame menu: a row for each preset with its size, and a frame of that size
  await page.click('[data-testid=frame-presets]');
  const rows = await page.locator('[role=menu] [role=menuitem]').allInnerTexts();
  assert.deepEqual(rows.map((r) => r.replace(/\s+/g, ' ')), ['Desktop 1440 × 900', 'Desktop HD 1920 × 1080', 'Tablet 768 × 1024', 'Mobile 390 × 844', 'Phone small 375 × 667', 'A4 595 × 842', 'Letter 612 × 792'], `${at}: the rows of the frame menu`);
  await page.click('[data-testid=preset-a4]');
  await page.locator('[role=menu]').waitFor({ state: 'detached' });
  assert.deepEqual(await frames(), [['a4', ...sizes.a4]], `${at}: the A4 preset makes a 595 x 842 frame`);
  await page.click('[data-testid=frame-presets]');
  await page.click('[data-testid=preset-desktopHd]');
  assert.deepEqual((await frames())[1], ['desktopHd', ...sizes.desktopHd], `${at}: Desktop HD`);

  // Ctrl+K opens the palette: a modal dialog with the focus in its field and every entry listed
  await page.focus('[data-testid=tool-text]');
  await open();
  assert.equal(await palette.getAttribute('role'), 'dialog', `${at}: role`);
  assert.equal(await palette.getAttribute('aria-modal'), 'true', `${at}: modal`);
  assert.match(await palette.getAttribute('aria-labelledby') ?? '', /./, `${at}: the dialog has a name`);
  assert.equal(await active(), 'palette-input', `${at}: the focus is in the field`);
  // 25 entries: 5 tools, 3 edit, 7 frame presets, 5 zoom, 2 panels, 3 more (export, variant, shortcuts).
  assert.equal(await options.count(), 25, `${at}: entries listed`);
  assert.equal(await page.getAttribute('[data-testid=palette-input]', 'aria-activedescendant'), 'palette-option-tool-select', `${at}: the first entry is highlighted`);
  // the focus is trapped
  for (let i = 0; i < 4; i++) await page.keyboard.press('Tab');
  assert.equal(await page.evaluate(() => !!document.activeElement?.closest('[data-testid=palette]')), true, `${at}: Tab leaves the palette`);
  await page.focus('[data-testid=palette-input]');
  // typing narrows the list; the best match is first and highlighted
  await page.keyboard.type('frame');
  const ids = await options.evaluateAll((es) => es.map((e) => e.getAttribute('data-id')));
  assert.equal(ids[0], 'tool-frame', `${at}: the best match first (${ids})`);
  assert.ok(ids.includes('preset-a4') && !ids.includes('undo'), `${at}: the list is narrowed`);
  assert.equal(await page.getAttribute('[data-testid=palette-input]', 'aria-activedescendant'), 'palette-option-tool-frame', `${at}: the best match is highlighted`);
  assert.match(await page.locator('[role=status]').filter({ hasText: 'result' }).innerText(), new RegExp(`^${ids.length} results?$`), `${at}: the count is announced`);
  await page.fill('[data-testid=palette-input]', 'xyzzy');
  assert.equal(await options.count(), 0, `${at}: no match`);
  assert.equal(await page.locator('[data-testid=palette-empty]').count(), 1, `${at}: says nothing matches`);
  await page.keyboard.press('Enter');
  assert.equal(await palette.count(), 1, `${at}: Enter with no match keeps the palette open`);
  // Escape closes and the focus returns to where it was
  await shut();
  assert.equal(await active(), 'tool-text', `${at}: the focus returns to the tool button (${await active()})`);

  // Enter runs the highlighted entry and closes the palette
  await open();
  await page.keyboard.type('rectangle');
  await page.keyboard.press('Enter');
  await palette.waitFor({ state: 'detached' });
  assert.equal(await tool(), 'rectangle', `${at}: Enter ran the Rectangle tool`);
  assert.equal(await active(), 'tool-text', `${at}: the focus is back after a run`);

  // the arrows choose, skipping what cannot run (Redo is disabled on this page)
  await open();
  await page.keyboard.type('do');
  assert.deepEqual(await options.evaluateAll((es) => es.map((e) => [e.getAttribute('data-id'), e.getAttribute('aria-disabled')])), [['undo', 'false'], ['redo', 'true']], `${at}: Undo can run, Redo is listed and disabled`);
  await page.keyboard.press('ArrowDown');
  assert.equal(await page.getAttribute('[data-testid=palette-input]', 'aria-activedescendant'), 'palette-option-undo', `${at}: ArrowDown skips the disabled entry`);
  await page.fill('[data-testid=palette-input]', 'redo');
  await page.keyboard.press('Enter');
  assert.equal(await palette.count(), 1, `${at}: Enter on an entry that cannot run does nothing`);
  await page.fill('[data-testid=palette-input]', '');
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('ArrowDown');
  assert.equal(await page.getAttribute('[data-testid=palette-input]', 'aria-activedescendant'), 'palette-option-tool-rectangle', `${at}: two ArrowDowns`);
  await page.keyboard.press('ArrowUp');
  await page.keyboard.press('Enter');
  await palette.waitFor({ state: 'detached' });
  assert.equal(await tool(), 'frame', `${at}: the arrows chose the Frame tool`);

  // a click runs a row; the palette makes a frame of a preset and deletes it
  await open();
  await page.locator('[data-id=zoom-in]').click();
  await palette.waitFor({ state: 'detached' });
  assert.notEqual(await page.innerText('[data-testid=zoom]'), '100%', `${at}: a click ran Zoom in`);
  await open();
  await page.keyboard.type('new letter');
  await page.keyboard.press('Enter');
  await palette.waitFor({ state: 'detached' });
  assert.deepEqual((await frames()).at(-1), ['letter', ...sizes.letter], `${at}: the palette made a Letter frame`);
  const before = (await frames()).length;
  await open();
  await page.keyboard.type('delete');
  await page.keyboard.press('Enter');
  await palette.waitFor({ state: 'detached' });
  assert.equal((await frames()).length, before - 1, `${at}: the palette ran Delete`);

  // the key shown in the palette is the one the sheet lists, and Ctrl+K again closes
  await open();
  assert.match(await page.locator('[data-id=undo]').innerText(), /Ctrl\+Z/, `${at}: the key of Undo`);
  assert.match(await page.locator('[data-id=toggle-layers]').innerText(), /Hide layers panel[\s\S]*Alt\+1/, `${at}: a toggle says what it does`);
  if (shots && theme === 'dark') await page.screenshot({ path: `${shots}/palette-open.png` });
  await page.keyboard.press('Control+k');
  await palette.waitFor({ state: 'detached' });

  // an entry that opens the shortcut sheet: the sheet opens, lists the palette key, and the focus goes back to where it was
  await page.focus('[data-testid=tool-text]');
  await open();
  await page.keyboard.type('keyboard');
  await page.keyboard.press('Enter');
  await page.locator('[data-testid=shortcut-hint]').waitFor();
  assert.match(await page.locator('[data-testid=shortcut-hint]').innerText(), /Command palette[\s\S]*Ctrl\+K/, `${at}: the sheet lists the palette key`);
  await page.keyboard.press('Escape');
  await page.locator('[data-testid=shortcut-hint]').waitFor({ state: 'detached' });
  assert.equal(await active(), 'tool-text', `${at}: the focus returns from the sheet to where the palette was opened (${await active()})`);

  // plain keys typed in the field are text: they do not choose a tool
  await open();
  await page.keyboard.type('vrt');
  assert.equal(await page.inputValue('[data-testid=palette-input]'), 'vrt', `${at}: typing is text`);
  assert.equal(await tool(), 'frame', `${at}: typing in the palette changed the tool`);
  await shut();

  // Ctrl+K in a text field does not open the palette, and what is typed there stays there
  await page.focus('[data-testid=demo-field]');
  await page.keyboard.press('Control+k');
  await page.keyboard.type('vf');
  assert.equal(await palette.count(), 0, `${at}: Ctrl+K in a field opened the palette`);
  assert.equal(await page.inputValue('[data-testid=demo-field]'), 'vf', `${at}: the field lost what was typed`);
  assert.equal(await tool(), 'frame', `${at}: typing in a field changed the tool`);

  // Ctrl+K over an open menu does nothing: the menu stays and no second overlay opens
  await page.click('[data-testid=file-menu]');
  await page.locator('[role=menu]').waitFor();
  await page.keyboard.press('Control+k');
  assert.equal(await palette.count(), 0, `${at}: the palette opened over a menu`);
  assert.equal(await page.locator('[role=menu]').count(), 1, `${at}: the menu closed`);
  await page.keyboard.press('Escape');
  await page.locator('[role=menu]').waitFor({ state: 'detached' });
  await page.focus('[data-testid=tool-text]');
  await open();
  await shut();
  if (shots) await (async () => { await open(); await page.keyboard.type('zoom'); await page.screenshot({ path: `${shots}/palette-${theme}.png` }); await shut(); })();
  await page.context().close();
}

if (editor) {
  const page = await (await browser.newContext({ viewport: { width: 1440, height: 900 }, reducedMotion: 'reduce' })).newPage();
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.goto(editor);
  await page.waitForFunction(() => document.querySelector('[data-testid=connection]')?.getAttribute('data-state') === 'live', null, { timeout: 20000 });
  const palette = page.locator('[data-testid=palette]');
  const dialog = page.locator('[data-testid=export-dialog]');
  const open = async () => { await page.keyboard.press('Control+k'); await palette.waitFor(); await page.waitForFunction(() => document.activeElement?.getAttribute('data-testid') === 'palette-input'); };
  const active = () => page.evaluate(() => document.activeElement?.getAttribute('data-testid') ?? document.activeElement?.tagName);
  const inDialog = () => page.evaluate(() => !!document.activeElement?.closest('[data-testid=export-dialog]'));
  const tool = () => page.getAttribute('[data-testid=tool]', 'data-tool');
  await page.click('[data-testid=frame-presets]');
  await page.click('[data-testid=preset-mobile]'); // makes a frame and selects it: Export can run
  await page.waitForTimeout(500);
  // the Export entry opens the dialog with the focus inside it, and Escape gives the focus back to where it was
  await page.focus('[data-testid=tool-select]');
  await open();
  await page.keyboard.type('export');
  await page.keyboard.press('Enter');
  await dialog.waitFor();
  assert.equal(await inDialog(), true, 'editor: the focus is not in the Export dialog');
  assert.equal(await palette.count(), 0, 'editor: the palette stayed open');
  // Ctrl+K over the Export dialog does nothing
  await page.keyboard.press('Control+k');
  assert.equal(await palette.count(), 0, 'editor: the palette opened over the Export dialog');
  assert.equal(await dialog.count(), 1, 'editor: the Export dialog closed');
  await page.keyboard.press('Escape');
  await dialog.waitFor({ state: 'detached' });
  assert.equal(await active(), 'tool-select', `editor: the focus did not return (${await active()})`);
  // Ctrl+K in a field of the inspector is the field's; typed tool letters are text
  const name = page.locator('[data-testid=right-card] input[aria-label=Name]');
  await name.focus();
  await page.keyboard.press('Control+k');
  assert.equal(await palette.count(), 0, 'editor: Ctrl+K in the Name field opened the palette');
  const was = await tool();
  await page.keyboard.type('f');
  assert.equal(await tool(), was, 'editor: a letter typed in a field chose a tool');
  await page.keyboard.press('Escape');
  // the inline text editor: a text made with the Text tool, edited with Enter
  await page.click('[data-testid=tool-text]');
  const frame = await page.locator('canvas').first().boundingBox();
  await page.mouse.click(frame.x + frame.width / 2, frame.y + frame.height / 2);
  await page.waitForTimeout(500);
  await page.keyboard.press('Enter');
  const field = page.locator('[data-testid=text-editor]');
  await field.waitFor({ timeout: 4000 });
  await page.keyboard.press('Control+k');
  assert.equal(await palette.count(), 0, 'editor: Ctrl+K in the text editor opened the palette');
  await page.keyboard.press('Escape');
  await page.close();
}

await browser.close();
assert.deepEqual(errors, [], `page errors: ${errors}`);
console.log('palette-e2e: ok');
