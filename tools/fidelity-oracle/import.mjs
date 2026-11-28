// Oracle for the importer (E4.4 slice 8, docs/import-html.md section 5 point 4, docs/fidelity-contract.md section 7a).
//   roundTrip   an export imported back: the imported file laid out by the core against the original's rects (no browser), and the
//               imported file exported again and loaded in Chromium against its own rects.
//   pages       hand-written HTML (hfixtures.mjs) loaded as written in Chromium: the box of each element against the core's rect of the
//               node the importer made of it, matched through `source`.
import fs from 'node:fs';
import path from 'node:path';
import { diffRects, domRects, fontsReady, OUT, solve } from './lib.mjs';
import hfixtures from './hfixtures.mjs';

const LIMIT = 0.05;
const stat = (rows) => {
  const worst = rows.reduce((a, b) => (b.max > (a?.max ?? -1) ? b : a), null);
  return { nodes: rows.length, missing: rows.filter((r) => r.missing).length, max: worst?.max ?? 0, worst: worst?.id ?? null, bad: rows.filter((r) => r.max > LIMIT).map((r) => r.id).slice(0, 60) };
};

/** What the import of one export gave, as a record for the gate: the boxes against the original's, the kinds that changed
 *  (`id:from>to`: the losses of docs/import-html.md section 3.7 that show), what the import refused, and whether the counts agree. */
export function backRecord(expected, back, nodes, mutate = (b) => b) {
  back = mutate(back);
  if (back.error) return { error: back.error, kindLost: [], unsupported: back.unsupported ?? [] };
  const kinds = Object.fromEntries(nodes.map((n) => [n.id, n.type]));
  const actual = Object.fromEntries(back.nodes.map((n) => [n.id, n.rect]));
  return {
    ...stat(diffRects(expected, actual)),
    kindLost: back.nodes.filter((n) => kinds[n.id] !== n.kind).map((n) => `${n.id}:${kinds[n.id]}>${n.kind}`),
    unsupported: back.unsupported,
  };
}

/** The imported file's second export, loaded in `page`: its boxes against the core's rects of the imported file. */
export async function againRecord(page, url, back) {
  await page.goto(url);
  const fontOk = await fontsReady(page);
  return { ...stat(diffRects(back.againRects, await domRects(page))), fontOk };
}

// ---- hand-written pages ----
const FONT_FACE = `<style data-oracle>@font-face{font-family:'Inter Variable';src:url('/inter.woff2') format('woff2');font-weight:100 900;font-display:block}html{font-family:'Inter Variable'}</style>`;

/** The page as written, plus the font the oracle's text table was measured with (marked, so that it is not an element of the page). */
export function pageHtml(html) {
  if (!html.includes('<head>')) throw new Error('a hand-written fixture is a whole document with a <head>');
  return html.replace('<head>', `<head>${FONT_FACE}`);
}

/** The box of the element each source names, in page coordinates. `source` is `tag.class#n`, n counting the elements of the text in
 *  document order with `html` as 1; `body` is the page (the root frame): the `html` element. An anonymous run (`/text`) has no box of
 *  its own and is counted, not compared. */
function boxes(page, sources) {
  return page.evaluate((srcs) => {
    const all = [...document.querySelectorAll('*')].filter((e) => !e.hasAttribute('data-oracle'));
    const out = {};
    for (const [id, s] of Object.entries(srcs)) {
      let el = null;
      if (s === 'body') el = document.documentElement;
      else {
        const m = /^([a-z][a-z0-9-]*)(?:\.[^#]*)?#(\d+)(\/text)?$/.exec(s);
        if (!m || m[3]) { out[id] = m?.[3] ? 'anonymous' : 'unparsed'; continue; }
        el = all[Number(m[2]) - 1] ?? null;
        if (!el || el.tagName.toLowerCase() !== m[1]) { out[id] = 'unmatched'; continue; }
      }
      const r = el.getBoundingClientRect();
      out[id] = { x: r.x + scrollX, y: r.y + scrollY, w: r.width, h: r.height };
    }
    return out;
  }, sources);
}

/** Runs the hand-written pages. `mutate(answer)` may change what the runner said before it is compared (the sensitivity check). */
export async function pages({ browser, base, mp, only = [], log = () => {}, mutate = (a) => a }) {
  const docs = hfixtures.filter((d) => !only.length || only.includes(d.name));
  const { out, rounds } = await solve([], mp, new Map(), docs.map(({ name, html, width }) => ({ name, html, width })));
  log(`hand-written pages: text table settled in ${rounds} runs`);
  const records = [];
  for (const [i, doc] of docs.entries()) {
    const ans = mutate(out.imports[i]);
    const ctx = await browser.newContext({ viewport: { width: doc.width ?? 1440, height: 900 }, deviceScaleFactor: 1 });
    const page = await ctx.newPage();
    const dir = path.join(OUT, 'pages', 'h', doc.name);
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(path.join(dir, 'index.html'), pageHtml(doc.html));
    await page.goto(`${base}/out/pages/h/${doc.name}/index.html`);
    const fontOk = await fontsReady(page);
    const seen = await boxes(page, Object.fromEntries(ans.nodes.map((n) => [n.id, n.source])));
    const rows = [];
    let anonymous = 0;
    for (const n of ans.nodes) {
      const got = seen[n.id];
      if (got === 'anonymous') { anonymous++; continue; }
      const want = ans.rects[n.id];
      if (typeof got === 'string' || !want) { rows.push({ id: n.source, missing: true, max: Infinity }); continue; }
      const d = ['x', 'y', 'w', 'h'].map((k) => got[k] - want[k]);
      rows.push({ id: n.source, max: Math.max(...d.map(Math.abs)), d });
    }
    records.push({
      name: doc.name, error: ans.error, fontOk, anonymous, nodes: ans.nodes.length,
      ...stat(rows),
      deltas: Object.fromEntries(rows.filter((r) => r.max > LIMIT).map((r) => [r.id, r.d])),
      unsupported: ans.nodes.flatMap((n) => n.unsupported),
      approximated: ans.nodes.flatMap((n) => n.approximated),
    });
    await ctx.close();
  }
  return { files: records, rounds };
}
