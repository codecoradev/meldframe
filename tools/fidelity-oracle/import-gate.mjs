// The gate of the importer oracle (import.mjs): called by gate.mjs. Lines of text for each violation; see docs/fidelity-contract.md section 7a.
import { IMPORT_GAPS, IMPORT_LOSSES, KNOWN_GAPS } from './known-gaps.mjs';

export const IMPORT_LIMIT = 0.05;
/** A run with fewer hand-written pages than this is not a run of the importer oracle. */
export const PAGE_FLOOR = 12;

/** The round trip of one file (geometry.mjs `roundTrip`): the imported file's boxes against the original's, its own export in the browser, what the
 *  import refused, and the losses of the kinds. A fixture of KNOWN_GAPS is not judged (its export is outside the contract), and a fuzz file under rule G3
 *  has no page promise. */
export function roundTripFailures(d) {
  if (KNOWN_GAPS[d.name]) return [];
  const fail = [], lost = new Set();
  for (const [key, rt] of Object.entries(d.roundTrip ?? {})) {
    const at = `${d.name}: import of ${key}`;
    if (rt.error) { fail.push(`${at}: ${rt.error}`); continue; }
    if (rt.missing) fail.push(`${at}: ${rt.missing} node(s) of the export are not in the imported file`);
    if (rt.max > IMPORT_LIMIT) fail.push(`${at}: box off by ${rt.max.toFixed(3)} px at ${rt.worst} against the original (limit ${IMPORT_LIMIT})`);
    for (const u of rt.unsupported) fail.push(`${at}: the import refused a property of an export: ${u}`);
    const p = rt.page;
    if (!p) fail.push(`${at}: the imported file was not exported again`);
    else if (!(d.fuzz && d.unspecified)) {
      if (p.missing) fail.push(`${at}: ${p.missing} node(s) missing from the page of the imported file`);
      if (!p.fontOk && d.textNodes > 0) fail.push(`${at}: the font did not load on the page of the imported file`);
      if (p.max > IMPORT_LIMIT) fail.push(`${at}: the page of the imported file is off by ${p.max.toFixed(3)} px at ${p.worst} (limit ${IMPORT_LIMIT})`);
    }
    for (const k of rt.kindLost) lost.add(k);
  }
  if (!Object.keys(d.roundTrip ?? {}).length && !d.errors.length) fail.push(`${d.name}: no frame was imported back`);
  const known = new Set(IMPORT_LOSSES[d.name] ?? []);
  for (const k of lost) if (!known.has(k)) fail.push(`${d.name}: the import loses the kind of a node (${k}), which docs/import-html.md section 3.7 does not list for it`);
  for (const k of known) if (!lost.has(k)) fail.push(`${d.name}: known loss closed (${k}); remove it from IMPORT_LOSSES and update docs/import-html.md section 3.7 and docs/fidelity-contract.md`);
  return fail;
}

/** The hand-written pages: each element's box in Chromium against the core's rect of its node, with IMPORT_GAPS as the known differences. */
export function pageFailures(results) {
  const files = results.import?.files ?? [];
  const fail = [];
  if (!results.only && !results.geometryOnly) {
    if (files.length < PAGE_FLOOR) fail.push(`only ${files.length} hand-written pages ran (at least ${PAGE_FLOOR})`);
    const names = new Set(files.map((f) => f.name));
    for (const g of Object.keys(IMPORT_GAPS)) if (!names.has(g)) fail.push(`known import gap ${g} is not a hand-written page any more: remove it from known-gaps.mjs`);
  }
  for (const f of files) {
    const problems = [];
    if (f.error) problems.push({ kind: 'error', text: `the import made nothing (${f.error})` });
    if (!f.fontOk) problems.push({ kind: 'font', text: 'the font did not load' });
    if (!(f.nodes - f.anonymous > 0)) problems.push({ kind: 'error', text: 'no element was compared' });
    if (f.missing) problems.push({ kind: 'missing', text: `${f.missing} node(s) have no element for their source` });
    if (f.max > IMPORT_LIMIT) problems.push({ kind: 'box', text: `box off by ${f.max.toFixed(3)} px at ${f.worst} (limit ${IMPORT_LIMIT})`, nodes: f.bad });
    const gap = IMPORT_GAPS[f.name];
    if (!gap) { for (const p of problems) fail.push(`${f.name}: ${p.text}`); continue; }
    const why = [];
    if (!problems.length) why.push('closed');
    for (const p of problems) {
      if (p.kind !== 'box') why.push(`fails another way: ${p.text}`);
      else for (const n of p.nodes) if (!gap.nodes.includes(n)) why.push(`node ${n} is off and is not part of the stated cause`);
    }
    const listed = new Set([...f.unsupported, ...f.approximated]);
    for (const c of gap.codes) if (!listed.has(c)) why.push(`the importer no longer lists ${c}`);
    if (why.includes('closed')) fail.push(`${f.name}: known import gap closed (${gap.reason}); remove it from IMPORT_GAPS and update docs/fidelity-contract.md`);
    else for (const w of [...new Set(why)]) fail.push(`${f.name}: known import gap no longer as stated: ${w}`);
  }
  return fail;
}
