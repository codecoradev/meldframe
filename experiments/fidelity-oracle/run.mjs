// Oracle A: export HTML/Tailwind vs core rects, per node. Usage: node run.mjs [fixtureName...]
import fs from 'node:fs';
import path from 'node:path';
import { serve, launch, openMeasurePage, cli, domRects, diffRects, OUT, RESULTS, HERE, BIN } from './lib.mjs';
import { compileTailwind } from './tailwind.mjs';
import { overlayHtml } from './overlay.mjs';
import { normalize } from './normalize.mjs';
import fixtures, { fuzzFixtures } from './fixtures.mjs';

const args = process.argv.slice(2);
const FUZZ = args.includes('--fuzz');
const NORM = args.includes('--norm');
const SAFE = args.includes('--safe');
const WS = args.includes('--ws');
const TAG = (FUZZ ? 'fuzz' : 'A') + (NORM ? '.norm' : '') + (SAFE ? '.safe' : '') + (WS ? '.ws' : '');
const only = args.filter((a) => !a.startsWith('--'));
const list = FUZZ ? fuzzFixtures : fixtures;
fs.mkdirSync(OUT, { recursive: true });
fs.mkdirSync(RESULTS, { recursive: true });
fs.copyFileSync('/dev/null', path.join(OUT, '.keep'));
if (!fs.existsSync(BIN)) throw new Error('build spike-export first: see README');

const { base, close } = await serve();
const browser = await launch();
const mp = await openMeasurePage(browser, base);
const summary = [];
const stat = (rows) => {
  const ok = rows.filter((r) => !r.missing);
  const maxRow = ok.reduce((a, b) => (b.max > (a?.max ?? -1) ? b : a), null);
  const over = (t) => ok.filter((r) => r.max > t).length;
  return { nodes: rows.length, missing: rows.filter((r) => r.missing).length, maxDelta: maxRow?.max ?? 0, worst: maxRow?.id ?? null, gt0001: over(0.001), gt05: over(0.5), gt1: over(1), gt2: over(2) };
};

async function render(html, name, viewport, dpr) {
  const f = path.join(OUT, name);
  fs.writeFileSync(f, html);
  const ctx = await browser.newContext({ viewport, deviceScaleFactor: dpr });
  const page = await ctx.newPage();
  await page.goto(`${base}/out/${name}`);
  await page.evaluate(() => document.fonts.ready);
  const fontOk = await page.evaluate(() => document.fonts.check("16px 'Inter Variable'"));
  const rects = await domRects(page);
  await ctx.close();
  return { rects, fontOk };
}

for (const fx of list) {
  if (only.length && !only.includes(fx.name)) continue;
  const nodes = NORM ? normalize(fx.nodes) : fx.nodes;
  const ts = await mp.layout(nodes);
  const nodesPath = path.join(OUT, `${fx.name}.nodes.json`);
  const tablePath = path.join(OUT, `${fx.name}.measure.json`);
  fs.writeFileSync(nodesPath, JSON.stringify(nodes));
  fs.writeFileSync(tablePath, JSON.stringify(ts.table));
  const rustR = cli(nodesPath, tablePath, 'rects');
  const rec = { name: fx.name, desc: fx.desc, nodes: nodes.length, textNodes: nodes.filter((n) => n.type === 'text').length, tableConflicts: ts.conflicts };
  if (rustR.status !== 0) { rec.error = `rects: ${rustR.err.slice(0, 200)}`; summary.push(rec); continue; }
  const rust = JSON.parse(rustR.out);
  rec.rustVsTs = stat(diffRects(ts.rects, rust));
  // the "canvas" rect = TS oracle with weight-aware Pixi measurer (identical to the WASM core by the spike's own tests)
  const expected = ts.rects;
  const variants = {};
  for (const fmt of ['html', 'tailwind']) {
    const ex = cli(nodesPath, tablePath, fmt, fmt === 'tailwind' ? ['--css-href', 'x.css'] : []);
    if (ex.status !== 0) { variants[fmt] = { error: ex.err.slice(0, 300) }; continue; }
    let html = ex.out;
    if (SAFE) html = fmt === 'html' ? html.replace(/justify-content:center/g, 'justify-content:safe center').replace(/justify-content:flex-end/g, 'justify-content:safe flex-end') : html.replace(/\bjustify-center\b/g, '[justify-content:safe_center]').replace(/\bjustify-end\b/g, '[justify-content:safe_flex-end]');
    if (WS && fmt === 'html') html = html.replace(/white-space:nowrap/g, 'white-space:pre').replace(/white-space:pre-line/g, 'white-space:pre-wrap;word-break:keep-all').replace('</style>', 'p{white-space:pre-wrap;word-break:keep-all}\n</style>');
    if (WS && fmt === 'html') {
      for (const n of nodes.filter((q) => q.type === 'text' && q.text.includes('\n'))) {
        const lh = await mp.lineHeight(n.fontSize, n.fontWeight);
        html = html.replace(new RegExp(`(\\.n-${n.id.replace(/[^A-Za-z0-9_-]/g, '_')}\\{[^}]*?)line-height:[0-9.]+px`), `$1line-height:${lh}px`);
      }
    }
    const warnings = ex.warnings;
    if (fmt === 'tailwind') html = html.replace('<link rel="stylesheet" href="x.css">', `<style>${await compileTailwind(html)}</style>`);
    html = html.replace("url('inter.woff2')", "url('/font/inter.woff2')");
    const runs = {};
    for (const [label, vp, dpr] of [['1280@1', { width: 1280, height: 900 }, 1], ['700@1', { width: 700, height: 500 }, 1], ['1280@2', { width: 1280, height: 900 }, 2]]) {
      const r = await render(html, `${fx.name}.${fmt}.${label.replace('@', '_')}.html`, vp, dpr);
      runs[label] = { fontOk: r.fontOk, ...stat(diffRects(expected, r.rects)) };
      if (label === '1280@1') runs.rows = diffRects(expected, r.rects);
    }
    variants[fmt] = { warnings, runs: Object.fromEntries(Object.entries(runs).filter(([k]) => k !== 'rows')), rows: runs.rows, html };
  }
  rec.asExported = { html: variants.html?.runs, tailwind: variants.tailwind?.runs, warnings: variants.html?.warnings };
  // Tailwind vs plain CSS: rect and computed style equivalence
  if (variants.html?.rows && variants.tailwind?.rows) {
    rec.twVsCss = stat(variants.html.rows.map((a, i) => { const b = variants.tailwind.rows[i]; return a.missing || b.missing ? { id: a.id, missing: true } : { id: a.id, max: Math.max(...['x', 'y', 'w', 'h'].map((k) => Math.abs(a.act[k] - b.act[k]))) }; }));
  }
  if (FUZZ) { rec.expRects = expected; rec.nodesUsed = nodes; }
  rec.rows = (FUZZ ? variants.html?.rows?.filter((r) => r.max > 0.5 || r.missing) : variants.html?.rows)?.map((r) => ({ id: r.id, max: r.max, d: r.d, exp: r.exp, act: r.act, text: nodes.find((n) => n.id === r.id)?.type === 'text' }));
  // overlay: candidate contract mapping for properties crates/export does not emit
  if (fx.overlay && variants.html?.html) {
    const oh = await overlayHtml(variants.html.html, nodes, base);
    const r = await render(oh, `${fx.name}.overlay.html`, { width: 1280, height: 900 }, 1);
    rec.withOverlay = stat(diffRects(expected, r.rects));
    rec.overlayRows = diffRects(expected, r.rects).map((x) => ({ id: x.id, max: x.max, d: x.d }));
  }
  summary.push(rec);
  const m = rec.asExported.html?.['1280@1'];
  console.log(`${fx.name.padEnd(20)} nodes=${rec.nodes} rust-vs-ts=${rec.rustVsTs.maxDelta.toFixed(4)} html max=${m?.maxDelta?.toFixed(3)} (${m?.worst}) >1px=${m?.gt1} tw-vs-css=${rec.twVsCss?.maxDelta?.toFixed(4)} overlay max=${rec.withOverlay?.maxDelta?.toFixed(3) ?? '-'}`);
}
fs.writeFileSync(path.join(FUZZ ? OUT : RESULTS, only.length ? `oracle-${TAG}.partial.json` : `oracle-${TAG}.json`), JSON.stringify(summary, null, 1));
await browser.close();
close();
