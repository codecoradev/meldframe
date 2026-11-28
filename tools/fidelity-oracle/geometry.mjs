// Oracle A: the boxes of the export in Chromium against the rects of the layout core, in both forms, and the Tailwind form
// against the plain CSS form.
import { diffRects, domRects, fontsReady, SETTINGS, solve, writePage } from './lib.mjs';
import fixtures from './fixtures.mjs';
import { fuzzFixtures } from './fuzz.mjs';
import { againRecord, backRecord } from './import.mjs';

/** Rule G3 (docs/fidelity-contract.md section 5), judged from the core's rects: a layout frame smaller than its own padding, children
 *  that need more than the content box of a frame that does not hug its main axis, a fill that resolves to under 1 px. The
 *  contract gives no promise there. The export reports the same cases as `layout_findings` but skips every frame that holds a
 *  hug text, so the oracle does not rely on its report. */
export function overflowing(nodes, rects) {
  const kids = new Map();
  for (const n of nodes) if (n.parentId) (kids.get(n.parentId) ?? kids.set(n.parentId, []).get(n.parentId)).push(n);
  for (const n of nodes) {
    if (n.type !== 'frame' || !n.layout || n.layout === 'none' || !rects[n.id]) continue;
    const r = rects[n.id], row = n.layout === 'row';
    const side = (k) => n[`pad${k}`] ?? n.padding ?? 16;
    const padH = side('L') + side('R'), padV = side('T') + side('B');
    const flow = (kids.get(n.id) ?? []).filter((c) => !c.absolute);
    if (r.w < padH || r.h < padV) return true;
    if ((row ? n.sw : n.sh) !== 'hug' && flow.length) {
      let used = (n.gap ?? 8) * (flow.length - 1);
      for (const c of flow) used += row ? rects[c.id].w + (c.marL ?? 0) + (c.marR ?? 0) : rects[c.id].h + (c.marT ?? 0) + (c.marB ?? 0);
      if (used > (row ? r.w - padH : r.h - padV) + 1e-6) return true;
    }
    for (const c of flow) {
      const main = row ? rects[c.id].w : rects[c.id].h, cross = row ? rects[c.id].h : rects[c.id].w;
      if (((row ? c.sw : c.sh) === 'fill' && main < 1) || ((row ? c.sh : c.sw) === 'fill' && cross < 1)) return true;
    }
  }
  return false;
}

const stat = (rows) => {
  const worst = rows.reduce((a, b) => (b.max > (a?.max ?? -1) ? b : a), null);
  return { nodes: rows.length, missing: rows.filter((r) => r.missing).length, max: worst?.max ?? 0, worst: worst?.id ?? null, bad: rows.filter((r) => r.max > 0.05).map((r) => r.id).slice(0, 60) };
};

/** The ids of a frame and everything below it. */
function subtree(nodes, root) {
  const ids = new Set([root]);
  for (let grew = true; grew;) {
    grew = false;
    for (const n of nodes) if (n.parentId && ids.has(n.parentId) && !ids.has(n.id)) { ids.add(n.id); grew = true; }
  }
  return ids;
}

/** Runs the fixtures and `seeds` fuzz files. Returns one record per file. */
/** `mutate(form, exported)` may change an export before it is loaded: the sensitivity check (sensitivity.mjs) uses it to prove that the gate fails on a wrong export. */
/** `mutateBack(form, back)` does the same to what the importer made of an export (import.mjs). */
export async function geometry({ browser, base, mp, seeds, only = [], log = () => {}, mutate = (_form, e) => e, mutateBack = (_form, b) => b }) {
  const docs = [...fixtures, ...fuzzFixtures(seeds)].filter((d) => !only.length || only.includes(d.name));
  const { out, table, rounds } = await solve(docs.map(({ name, nodes, variables, modes }) => ({ name, nodes, variables, modes })), mp);
  log(`text table settled in ${rounds} runs, ${table.size} measurements`);
  const contexts = await Promise.all(SETTINGS.map(async ([label, viewport, dpr]) => ({ label, ctx: await browser.newContext({ viewport, deviceScaleFactor: dpr }) })));
  const pages = Object.fromEntries(await Promise.all(contexts.map(async ({ label, ctx }) => [label, await ctx.newPage()])));
  const records = [];
  for (const [i, doc] of docs.entries()) {
    const ans = out.fixtures[i];
    const rec = { name: doc.name, fuzz: !!doc.fuzz, nodes: doc.nodes.length, textNodes: doc.nodes.filter((n) => n.type === 'text').length, errors: ans.errors, frames: [], warnings: [], unsupported: [], runs: {}, twVsCss: null, roundTrip: {} };
    for (const fr of ans.frames) {
      const ids = subtree(doc.nodes, fr.id);
      const expected = Object.fromEntries(Object.entries(ans.rects).filter(([id]) => ids.has(id)));
      rec.frames.push(fr.id);
      const at1280 = {};
      for (const form of ['css', 'tailwind']) {
        const exp = fr[form] && mutate(form, fr[form]);
        if (!exp) continue;
        rec.warnings.push(...exp.warnings);
        rec.unsupported.push(...exp.unsupported);
        const url = await writePage(base, `${doc.name}/${fr.id}.${form}`, form, exp);
        // the export imported back: its boxes against the original's, and its own export in the browser (import.mjs)
        if (exp.back) {
          const back = mutateBack(form, exp.back);
          const rt = backRecord(expected, back, doc.nodes);
          if (back.again) rt.page = await againRecord(pages['1280@1'], await writePage(base, `${doc.name}/${fr.id}.${form}.back`, form, back.again), back);
          rec.roundTrip[`${fr.id} ${form}`] = rt;
        }
        // fuzz files are checked at one setting, the hand-written fixtures at all three (contract section 7)
        for (const [label] of doc.fuzz ? SETTINGS.slice(0, 1) : SETTINGS) {
          const page = pages[label];
          await page.goto(url);
          const fontOk = await fontsReady(page);
          const rects = await domRects(page);
          const rows = diffRects(expected, rects);
          const key = `${form}@${label}`;
          const s = { ...stat(rows), fontOk };
          const prev = rec.runs[key];
          const top = prev && prev.max >= s.max ? prev : s;
          rec.runs[key] = { ...top, nodes: (prev?.nodes ?? 0) + s.nodes, missing: (prev?.missing ?? 0) + s.missing, fontOk: (prev?.fontOk ?? true) && s.fontOk, bad: [...(prev?.bad ?? []), ...s.bad] };
          if (label === '1280@1') at1280[form] = rects;
        }
      }
      if (at1280.css && at1280.tailwind) {
        const rows = Object.keys(expected).map((id) => {
          const a = at1280.css[id], b = at1280.tailwind[id];
          return !a || !b ? { id, missing: true, max: Infinity } : { id, max: Math.max(...['x', 'y', 'w', 'h'].map((k) => Math.abs(a[k] - b[k]))) };
        });
        const s = stat(rows);
        rec.twVsCss = rec.twVsCss && rec.twVsCss.max >= s.max ? rec.twVsCss : s;
      }
    }
    rec.warnings = [...new Set(rec.warnings)];
    // a fuzz file has had rule G1 applied (fuzz.mjs): the export's fill_under_hug warning on one is a failure the gate reports
    rec.fillUnderHug = rec.warnings.some((w) => w.startsWith('fill_under_hug:'));
    // outside the contract: rule G3, judged from the core's rects
    rec.g3 = overflowing(doc.nodes, ans.rects);
    rec.unspecified = rec.g3;
    // G3 that the export did not warn about: the finding of layout_findings skips frames with a hug text
    rec.silentG3 = rec.g3 && !rec.warnings.some((w) => /^(overflow|smaller_than_padding|zero_fill):/.test(w));
    if (rec.fuzz && Object.values(rec.runs).some((r) => r.max > 0.05)) rec.nodesJson = doc.nodes;
    records.push(rec);
  }
  await Promise.all(contexts.map(({ ctx }) => ctx.close()));
  return { files: records, rounds, measurements: table.size };
}
