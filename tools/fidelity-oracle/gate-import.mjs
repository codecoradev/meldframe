// The gate of the importer's oracle (import.mjs): the violations in `results.import`, as lines of text, and the known gaps of the importer
// (IMPORT_GAPS in known-gaps.mjs) checked in both directions as gate.mjs checks KNOWN_GAPS.
import { IMPORT_GAPS } from './known-gaps.mjs';
import { EXPORT_REFUSED } from './import.mjs';

export const IMPORT_LIMITS = { geometry: 0.05, pixels: 0 };
/** A run that checks less than this is not a run of the importer's oracle: pages outside the gaps, round trips (a frame in a form), and the
 *  pages whose pixels are compared. */
export const IMPORT_FLOORS = { pages: 12, gapPages: 10, roundTrips: 40, pixelPages: 9 };

const first = (list, n = 3) => list.slice(0, n).join('; ');

/** What is wrong with a page, as problems with a kind: 'error', 'font', 'missing', 'ambiguous' are never excused; 'box' (with the nodes) and
 *  'pixels' are what a gap may state. */
function pageProblems(p) {
  const out = [];
  if (p.error) out.push({ kind: 'error', text: `the importer refused the page: ${p.error}` });
  if (p.missing.length) out.push({ kind: 'missing', text: `no imported node is named ${first(p.missing)}` });
  if (p.ambiguous.length) out.push({ kind: 'ambiguous', text: `more than one imported node is named ${first(p.ambiguous)}` });
  if (!p.fontOk && p.textNodes > 0) out.push({ kind: 'font', text: 'the font did not load' });
  if (p.max > IMPORT_LIMITS.geometry) out.push({ kind: 'box', nodes: p.bad, text: `box of ${p.worst} off by ${p.max.toFixed(3)} px against the browser (limit ${IMPORT_LIMITS.geometry}): ${first(p.detail.map((d) => `${d.name} browser ${d.browser} core ${d.core}`))}` });
  if (p.pixels && p.pixels.differing > IMPORT_LIMITS.pixels) out.push({ kind: 'pixels', text: `${p.pixels.differing} of ${p.pixels.total} pixels differ between the page and the export of its import` });
  return out;
}

/** The reasons a gap is not as its entry states, or ['closed'] when the page or the round trip has no problem at all. */
function gapWhy(gap, problems, findings) {
  const why = [];
  if (!problems.length) why.push('closed');
  for (const p of problems) {
    if (p.kind === 'box') {
      for (const n of p.nodes) if (!gap.nodes.includes(n)) why.push(`node ${n} is off and is not part of the stated cause`);
    } else if (!(p.kind === 'pixels' && gap.pixels)) why.push(`fails another way: ${p.text}`);
  }
  if (gap.warning && !findings.some((w) => w.startsWith(gap.warning))) why.push(`the importer no longer reports ${gap.warning}`);
  return why;
}

export function evaluateImport(results) {
  const fail = [];
  const imp = results.import;
  if (!imp) return ['the importer oracle ran no section (results.import is missing)'];
  const pages = imp.pages ?? [], rows = imp.roundTrips ?? [];
  const pageNames = new Set(pages.map((p) => p.name)), fixtureNames = new Set((results.geometry?.files ?? []).map((f) => f.name));
  if (!results.only) {
    if (pages.filter((p) => !(p.name in IMPORT_GAPS)).length < IMPORT_FLOORS.pages) fail.push(`fewer than ${IMPORT_FLOORS.pages} pages ran outside the known gaps`);
    if (pages.filter((p) => p.name in IMPORT_GAPS).length < IMPORT_FLOORS.gapPages) fail.push(`fewer than ${IMPORT_FLOORS.gapPages} known-gap pages ran`);
    if (pages.filter((p) => p.pixels).length < IMPORT_FLOORS.pixelPages) fail.push(`the pixels of fewer than ${IMPORT_FLOORS.pixelPages} pages were compared`);
    if (rows.filter((r) => !r.skipped).length < IMPORT_FLOORS.roundTrips) fail.push(`fewer than ${IMPORT_FLOORS.roundTrips} round trips ran`);
    for (const [name, gap] of Object.entries(IMPORT_GAPS)) {
      if (!(gap.kind === 'page' ? pageNames : fixtureNames).has(name)) fail.push(`known import gap ${name} is not a ${gap.kind === 'page' ? 'page' : 'fixture'} any more: remove it from known-gaps.mjs`);
    }
  }
  for (const p of pages) {
    const gap = IMPORT_GAPS[p.name], problems = pageProblems(p);
    if (!gap) { for (const q of problems) fail.push(`${p.name}: ${q.text}`); continue; }
    const why = gapWhy(gap, problems, p.findings);
    if (why.includes('closed')) fail.push(`${p.name}: known import gap closed (${gap.reason}); remove it from known-gaps.mjs and update docs/fidelity-contract.md`);
    else for (const w of [...new Set(why)]) fail.push(`${p.name}: known import gap no longer as stated: ${w}`);
  }
  for (const r of rows) {
    if (r.skipped) continue;
    const id = `${r.name} ${r.frame} (${r.form})`;
    const gap = IMPORT_GAPS[r.name];
    const problems = [];
    if (r.error) problems.push({ kind: 'error', text: `the importer refused the export: ${r.error}` });
    if (r.nodes !== r.original) problems.push({ kind: 'structure', text: `${r.original} nodes became ${r.nodes}` });
    for (const d of r.diffs) problems.push({ kind: 'field', text: d });
    if (r.max > IMPORT_LIMITS.geometry) problems.push({ kind: 'box', nodes: [r.worst], text: `box of ${r.worst} off by ${r.max.toFixed(3)} px after the round trip (limit ${IMPORT_LIMITS.geometry})` });
    if (r.fixedPoint) problems.push({ kind: 'fixed point', text: `export, import and export is not a fixed point: ${r.fixedPoint}` });
    for (const u of r.unsupported) if (!EXPORT_REFUSED.some((e) => u.startsWith(`${e}:`))) problems.push({ kind: 'refused', text: `the importer refused ${u} on an export` });
    if (!gap) { for (const q of problems) fail.push(`${id}: ${q.text}`); continue; }
    const why = gapWhy(gap, problems, []);
    if (why.includes('closed')) fail.push(`${r.name}: known import gap closed (${gap.reason}); remove it from known-gaps.mjs and update docs/fidelity-contract.md`);
    else for (const w of [...new Set(why)]) fail.push(`${id}: known import gap no longer as stated: ${w}`);
  }
  return fail;
}

/** The numbers of a run. */
export function reportImport(results, log) {
  const imp = results.import;
  if (!imp) return;
  const rows = imp.roundTrips.filter((r) => !r.skipped && !(r.name in IMPORT_GAPS));
  const worst = rows.reduce((m, r) => Math.max(m, r.max), 0);
  const pages = imp.pages.filter((p) => !(p.name in IMPORT_GAPS));
  log(`import: ${rows.length} round trips (${rows.reduce((n, r) => n + r.nodes, 0)} nodes) outside the gaps, largest box distance ${worst.toFixed(4)} px, fields that differ ${rows.reduce((n, r) => n + r.diffs.length, 0)}`);
  log(`  pages ${pages.length} outside the gaps (${pages.reduce((n, p) => n + p.compared, 0)} boxes against the browser, largest delta ${Math.max(0, ...pages.map((p) => p.max)).toFixed(4)} px), known gaps ${imp.pages.length - pages.length}, pixels differing ${pages.reduce((n, p) => n + (p.pixels?.differing ?? 0), 0)} of ${pages.reduce((n, p) => n + (p.pixels?.total ?? 0), 0)}`);
}
