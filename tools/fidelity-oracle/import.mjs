// Oracle E, the importer (E4.4, docs/fidelity-contract.md section 14). Two checks, both against the real thing:
//  - the round trip: the runner imports every exported frame back (both forms) and compares the imported file with the original (boxes
//    within 0.05 px by the layout core, the fields the export writes, the refused properties, the fixed point of export, import and
//    export); the export of the imported file is then loaded in Chromium like any other file (geometry.mjs, `imported` files);
//  - the pages: hand-written HTML is loaded as written in Chromium, and the box of every element that has a `data-name` is compared with
//    the layout core's rect of the imported node of that name.
import fs from 'node:fs';
import path from 'node:path';
import { compileTailwind, decodePng, diffPixels, fontsReady, OUT, solve } from './lib.mjs';
import { screenshot } from './screenshot.mjs';
import pages from './pages.mjs';
import { IMPORT_GAPS } from './known-gaps.mjs';

/** The properties an export always carries that the importer does not read, as it says: the reset's `::before` and `::after`, the one font
 *  family, and the pins of a text (`line-height`, and `word-break` for the wrapping form). Anything else refused on an export is a finding. */
export const EXPORT_REFUSED = ['::before', '::after', 'font-family', 'line-height', 'word-break'];

/** The fixtures that are not run as round-trip passes (docs/import-html.md section 5, item 3): the unspecified overflow regime (rule G3) and
 *  the known gaps of the export; f17 is checked the other way, in IMPORT_GAPS. */
export const ROUNDTRIP_SKIPPED = ['f13-overflow', 'f14-text-edge'];

const FONT_CSS = `<style>@font-face{font-family:'Inter Variable';src:url('/inter.woff2') format('woff2');font-weight:100 900;font-display:block}html{font-family:'Inter Variable'}</style>`;

/** The page as the browser gets it: the HTML as written in standards mode, with Inter and the extra css (read after the style blocks, as the
 *  importer reads it) added. Nothing else is changed. */
export function browserCopy(page) {
  let html = page.html;
  const extra = page.tailwind ? '<link rel="stylesheet" href="meldframe.css">' : page.css ? `<style>${page.css}</style>` : '';
  html = /<head[^>]*>/i.test(html) ? html.replace(/<head[^>]*>/i, (m) => m + FONT_CSS) : FONT_CSS + html;
  html = /<\/body>/i.test(html) ? html.replace(/<\/body>/i, extra + '</body>') : html + extra;
  return /^\s*<!doctype/i.test(html) ? html : '<!doctype html>' + html;
}

/** The round-trip records of a run of the runner: one per exported frame and form. */
export function roundTrips(docs, out) {
  const rows = [];
  for (const [i, ans] of out.fixtures.entries()) {
    const doc = docs[i];
    for (const fr of ans.frames) for (const form of ['css', 'tailwind']) {
      const rt = fr[form]?.imported;
      if (!rt) continue;
      const bound = !!doc.variables?.variables?.length || Object.keys(doc.modes ?? {}).length > 0;
      rows.push({
        name: doc.name, frame: fr.id, form, fuzz: !!doc.fuzz, error: rt.error, nodes: rt.nodes.length, original: rt.original,
        max: rt.max_delta, worst: rt.worst, diffs: rt.diffs, unsupported: rt.unsupported,
        // a binding comes back as its literal (docs/import-html.md section 3.7), so the export of the import is not the export imported
        fixedPoint: bound ? null : rt.fixed_point, fixedPointSkipped: bound,
        skipped: ROUNDTRIP_SKIPPED.includes(doc.name),
      });
    }
  }
  return rows;
}

/** The files imported from the css form of each export, as files for the geometry oracle: their export is loaded in Chromium and compared with the
 *  core's rects of the import (the Tailwind form's import is the same file; it is compared field by field in the round trip). */
export function importedDocs(docs, out) {
  const list = [];
  for (const [i, ans] of out.fixtures.entries()) {
    if (ROUNDTRIP_SKIPPED.includes(docs[i].name) || docs[i].name in IMPORT_GAPS) continue;
    for (const fr of ans.frames) {
      const rt = fr.css?.imported;
      if (rt && !rt.error && rt.nodes.length) list.push({ name: `imp-${docs[i].name}-${fr.id}`, nodes: rt.nodes, imported: true });
    }
  }
  return list;
}

const domNamed = (page) =>
  page.evaluate(() => [...document.querySelectorAll('[data-name]')].map((el) => {
    const r = el.getBoundingClientRect();
    return { name: el.getAttribute('data-name'), x: r.x + scrollX, y: r.y + scrollY, w: r.width, h: r.height };
  }));

/** Loads each page in Chromium and compares the boxes of its named elements with the rects of the imported nodes of those names. */
/** `mutate(answer)` may change what the importer returned before it is compared: the sensitivity check (sensitivity.mjs) uses it to prove that the gate fails on a wrong import. */
export async function pagesOracle({ browser, base, mp, only = [], log = () => {}, mutate = (a) => a }) {
  const list = pages.filter((p) => !only.length || only.includes(p.name));
  const { out, table, rounds } = await solve([], mp, new Map(), list.map(({ name, html, css, width }) => ({ name, html, css: css ?? '', width })));
  log(`pages: text table settled in ${rounds} runs, ${table.size} measurements`);
  const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  const tab = await ctx.newPage();
  const records = [];
  for (const [i, p] of list.entries()) {
    const ans = mutate(out.pages[i]);
    const rec = { name: p.name, error: ans.error, compared: 0, missing: [], ambiguous: [], max: 0, worst: null, bad: [], detail: [], fontOk: true, textNodes: ans.nodes.filter((n) => n.type === 'text').length, nodes: ans.nodes.length, findings: [...ans.unsupported, ...Object.keys(ans.report).map((c) => `report:${c}`)], approximated: ans.approximated };
    records.push(rec);
    if (ans.error) continue;
    const dir = path.join(OUT, 'pages', `page-${p.name}`);
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(path.join(dir, 'index.html'), browserCopy(p));
    if (p.tailwind) fs.writeFileSync(path.join(dir, 'meldframe.css'), await compileTailwind(p.css ?? '', p.html));
    await tab.setViewportSize({ width: p.width ?? 1440, height: 900 });
    await tab.goto(`${base}/out/pages/page-${p.name}/index.html`);
    rec.fontOk = await fontsReady(tab);
    if (p.pixels && ans.exported) rec.pixels = await pixels({ browser, base, tab, p, ans });
    const byName = new Map();
    for (const n of ans.nodes) byName.set(n.name, [...(byName.get(n.name) ?? []), n.id]);
    for (const el of await domNamed(tab)) {
      const ids = byName.get(el.name) ?? [];
      if (!ids.length) { rec.missing.push(el.name); continue; }
      if (ids.length > 1) { rec.ambiguous.push(el.name); continue; }
      const r = ans.rects[ids[0]];
      const max = Math.max(...['x', 'y', 'w', 'h'].map((k) => Math.abs(el[k] - r[k])));
      rec.compared++;
      if (max > rec.max) { rec.max = max; rec.worst = el.name; }
      if (max > 0.05) { rec.bad.push(el.name); rec.detail.push({ name: el.name, browser: [el.x, el.y, el.w, el.h].map((v) => +v.toFixed(3)), core: [r.x, r.y, r.w, r.h].map((v) => +v.toFixed(3)) }); }
    }
  }
  await ctx.close();
  const imported = list.flatMap((p, i) => (out.pages[i].error ? [] : [{ name: `imp-${p.name}`, nodes: out.pages[i].nodes, imported: true }]));
  return { pages: records, imported, rounds, measurements: table.size };
}

/** The pixels of the page against the pixels of the export of what was imported, inside the box of the imported root: how many differ. A page
 *  with no text, so that nothing but fills, borders, radii, shadows and gradients is compared (a text is checked by its box). */
async function pixels({ browser, base, tab, p, ans }) {
  const root = ans.rects[ans.nodes[0].id];
  const clip = { x: Math.floor(root.x), y: Math.floor(root.y), width: Math.ceil(root.w), height: Math.ceil(root.h) };
  const want = await decodePng(browser, await screenshot(tab, { clip, fullPage: true }));
  const dir = path.join(OUT, 'pages', `page-${p.name}`);
  fs.writeFileSync(path.join(dir, 'export.html'), ans.exported);
  const ctx = await browser.newContext({ viewport: { width: p.width ?? 1440, height: 900 } });
  const other = await ctx.newPage();
  await other.goto(`${base}/out/pages/page-${p.name}/export.html`);
  await fontsReady(other);
  const got = await decodePng(browser, await screenshot(other, { clip, fullPage: true }));
  await ctx.close();
  return { differing: diffPixels(want, got), total: clip.width * clip.height };
}
