// Browser check of the host text measurer: node packages/web-e2e/scripts/measurer-e2e.mjs [web-url]
// Needs the Vite dev server (`npm run dev` in apps/web; no Meldframe server is involved), the WASM bridge built (`just wasm-build`)
// and Playwright from the global module path. Hand-run, not in CI: it needs a browser and real fonts.
//
// In a real Chromium, with the Inter the page loads, it lays out hug texts with the app layer's flag on, first with the stub (the
// default measurer) and then with the measurer backed by Pixi's `CanvasTextMetrics` that the web client injects, and checks
// what the canvas would draw: that Pixi does not wrap a hug text in its laid-out box, that the box is the Pixi width within
// 1 px, that the weight reaches the width, and that the scene is laid out again once the fonts have loaded. The stub is
// printed beside it: half an em a character is off for Inter, and for wide capitals too narrow, so Pixi wraps the text in its box.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const { chromium } = createRequire(import.meta.url)('playwright');
const base = process.argv[2] ?? 'http://localhost:5173';
// The page serves source files by path, so only a server on this machine is used.
if (!['localhost', '127.0.0.1', '[::1]'].includes(new URL(base).hostname)) {
  console.error(`measurer-e2e: ${base} is not on this machine`);
  process.exit(2);
}
const root = new URL('../../../', import.meta.url).pathname;
const results = [];
const check = (name, fn) => {
  try { fn(); results.push(true); console.log('ok  ', name); } catch (e) { results.push(false); console.log('FAIL', name, '\n    ', String(e.message ?? e).split('\n').join('\n     ')); }
};

const browser = await chromium.launch();
try {
  const page = await browser.newPage({ viewport: { width: 1200, height: 800 } });
  const errors = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto(`${base}/?tokens`); // the design tokens page: it needs no server and loads the page's CSS, which declares Inter
  const out = await page.evaluate(async (root) => {
    const { Engine, loadWasm } = await import(`/@fs${root}packages/web-link/src/engine.ts`);
    const { installPixiMeasurer, pixiMetrics } = await import(`/@fs${root}apps/web/src/canvas/pixitext.ts`);
    const { resolveScene } = await import(`/@fs${root}packages/web-canvas/src/scene.ts`);
    const { WorldView } = await import(`/@fs${root}apps/web/src/canvas/stage.ts`);
    await loadWasm();
    const text = (id, pos, t, fontWeight, extra = {}) => ({ id, type: 'text', pos, parentId: 'f', text: t, fontSize: 20, fontWeight, sw: 'hug', sh: 'hug', ...extra });
    const nodes = [
      { id: 'f', type: 'frame', pos: 'a0', w: 900, h: 300, layout: 'column', padding: 10, gap: 10 },
      text('r', 'a1', 'Design with your agent, in the open', 400),
      text('b', 'a2', 'Design with your agent, in the open', 700),
      text('w', 'a3', 'Wrapped inside a box one hundred and fifty pixels wide', 400, { sw: 'fixed', w: 150, textAlign: 'center' }),
      text('c', 'a4', 'WWWW MMMM WIDE', 700), // wide capitals: half an em a character is far too narrow for them
    ];
    const engine = Engine.create({ actor: 'usr_me', nodes, seq: 0, session: 1 });
    const boxes = () => Object.fromEntries(engine.snapshot().scene.nodes.filter((n) => n.kind === 'text').map((n) => [n.id, { w: n.w, h: n.h }]));
    engine.setLayoutOnCanvas(true);
    const stub = boxes();
    // Pixi's own measurement of a text of these nodes on the style the canvas draws with, at a wrap width (null: one line); the
    // line count is the height over the height of one line. The stub box is what the canvas would draw in without the
    // measurer: Pixi wraps the text at the box width.
    const pixiOn = (n, wrapWidth) => {
      const one = pixiMetrics(n.text, { fontSize: n.fontSize, fontWeight: n.fontWeight, wrapWidth: null });
      const m = pixiMetrics(n.text, { fontSize: n.fontSize, fontWeight: n.fontWeight, wrapWidth });
      return { width: m.width, height: m.height, lines: Math.round(m.height / one.height), oneLine: one };
    };
    const stubLines = Object.fromEntries(nodes.slice(1).map((n) => [n.id, pixiOn(n, stub[n.id].w).lines]));
    // The measurer is injected before the font has loaded: the first answer is the stub's, then the scene is laid out again.
    let reads = 0;
    engine.subscribe(() => reads++);
    installPixiMeasurer(engine);
    const sameAsStub = () => nodes.slice(1).every((n) => boxes()[n.id].w === stub[n.id].w && boxes()[n.id].h === stub[n.id].h);
    const beforeFonts = sameAsStub();
    for (let i = 0; i < 100 && sameAsStub(); i++) await new Promise((r) => setTimeout(r, 50));
    const pixi = boxes();
    // What the canvas draws in those boxes: a Pixi text of the style the stage uses, in its laid-out box.
    const resolved = resolveScene(engine.snapshot().scene);
    const world = new WorldView();
    world.apply(resolved.scene, resolved.patch);
    const drawn = [];
    const walk = (o) => { if (typeof o.text === 'string' && 'style' in o) drawn.push(o); o.children?.forEach(walk); };
    walk(world.root);
    const rows = nodes.slice(1).map((n) => {
      const natural = pixiOn(n, null).oneLine;
      const inBox = pixiOn(n, pixi[n.id].w);
      return { id: n.id, stub: stub[n.id], stubLines: stubLines[n.id], box: pixi[n.id], naturalW: natural.width, naturalH: natural.height, pixiLines: inBox.lines };
    });
    const drawnWidths = drawn.map((t) => t.width);
    const drawnX = drawn.map((t) => t.x);
    return { rows, beforeFonts, reads, drawnWidths, drawnX, fontOk: document.fonts.check('400 16px "Inter Variable"') };
  }, root);

  console.log(JSON.stringify(out.rows.map((r) => ({ id: r.id, stubBox: r.stub, pixiBox: r.box, pixiNaturalWidth: +r.naturalW.toFixed(2), linesInStubBox: r.stubLines, linesInPixiBox: r.pixiLines })), null, 0));
  const [r, b, w, c] = out.rows;
  check('Inter is loaded in the page', () => assert.equal(out.fontOk, true));
  check('before the fonts are ready the engine lays out with the stub, then again with Pixi (the scene is read again)', () => {
    assert.equal(out.beforeFonts, true);
    assert.ok(out.reads >= 1, 'a re-read happened');
    assert.notDeepEqual(r.box, r.stub);
  });
  check('the stub box is not the width of the text in Inter: off by more than a pixel, and too narrow for wide capitals (Pixi wraps them in it)', () => {
    assert.ok(Math.abs(r.stub.w - r.naturalW) > 1, `${r.stub.w} vs ${r.naturalW}`);
    assert.ok(c.stub.w < c.naturalW - 1, `${c.stub.w} vs ${c.naturalW}`);
    assert.ok(c.stubLines > 1, `lines ${c.stubLines}`);
  });
  check('the Pixi measurer sizes the hug text to Pixi\'s own width within 1 px, and Pixi does not wrap it', () => {
    assert.ok(Math.abs(r.box.w - r.naturalW) <= 1, `${r.box.w} vs ${r.naturalW}`);
    assert.ok(Math.abs(b.box.w - b.naturalW) <= 1, `${b.box.w} vs ${b.naturalW}`);
    assert.ok(Math.abs(c.box.w - c.naturalW) <= 1, `${c.box.w} vs ${c.naturalW}`);
    assert.equal(r.pixiLines, 1);
    assert.equal(b.pixiLines, 1);
    assert.equal(c.pixiLines, 1);
    assert.ok(Math.abs(r.box.h - r.naturalH) <= 1, `${r.box.h} vs ${r.naturalH}`);
  });
  check('the weight reaches the width: the bold text is wider than the regular one', () => {
    assert.ok(b.box.w > r.box.w + 2, `${b.box.w} vs ${r.box.w}`);
  });
  check('a wrapped text takes its height from Pixi\'s line breaks at the width of its box', () => {
    assert.ok(w.pixiLines > 1, `lines ${w.pixiLines}`);
    assert.ok(Math.abs(w.box.h - w.pixiLines * (w.naturalH)) <= w.pixiLines, `${w.box.h} vs ${w.pixiLines} x ${w.naturalH}`);
  });
  check('the stage draws each hug text as wide as its laid-out box (within 1 px) and the wrapped one no wider than its box', () => {
    assert.equal(out.drawnWidths.length, 4);
    for (const [i, row] of out.rows.entries()) {
      const d = out.drawnWidths[i];
      if (row.id === 'w') assert.ok(d <= row.box.w + 1, `${row.id}: drawn ${d} in ${row.box.w}`);
      else assert.ok(Math.abs(d - row.box.w) <= 1, `${row.id}: drawn ${d} in ${row.box.w}`);
    }
  });
  check('a bold hug text is drawn wider than the regular one, as its box is, and a centred wrapped text is drawn centred in its box', () => {
    assert.ok(out.drawnWidths[1] > out.drawnWidths[0] + 2, `bold ${out.drawnWidths[1]} vs regular ${out.drawnWidths[0]}`);
    assert.equal(out.drawnX[0], 0, 'left aligned: at the left edge');
    assert.ok(Math.abs(out.drawnX[2] - (w.box.w - out.drawnWidths[2]) / 2) <= 0.5, `x ${out.drawnX[2]} for a block ${out.drawnWidths[2]} wide in ${w.box.w}`);
    assert.ok(out.drawnX[2] > 0, 'the block of lines is narrower than its box, so it is moved');
  });
  check('no page error', () => assert.deepEqual(errors, []));
} finally {
  await browser.close();
}
const failed = results.filter((ok) => !ok).length;
console.log(failed ? `${failed} of ${results.length} checks failed` : `all ${results.length} checks passed`);
process.exit(failed ? 1 : 0);
