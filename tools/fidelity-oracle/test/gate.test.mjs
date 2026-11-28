// The gate on made-up results: a violation and a known gap that closed must exit 1, a clean run and a standing gap exit 0.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { IMPORT_GAPS, IMPORT_LOSSES, KNOWN_GAPS } from '../known-gaps.mjs';
import { FLOORS, PAINT_FIXTURES } from '../gate.mjs';
import { PAGE_FLOOR } from '../import-gate.mjs';
import { overflowing } from '../geometry.mjs';

const gate = path.join(path.dirname(fileURLToPath(import.meta.url)), '../gate.mjs');
/** The import of one export: clean unless `over` says otherwise. */
const trip = (over = {}) => ({ nodes: 3, max: 0, worst: 'a', missing: 0, bad: [], kindLost: [], unsupported: [], page: { nodes: 3, max: 0, worst: 'imp-0', missing: 0, fontOk: true, bad: [] }, ...over });
const trips = (over) => ({ 'r css': trip(over), 'r tailwind': trip(over) });
const run = (name, max, extra = {}) => ({ name, fuzz: false, nodes: 3, textNodes: 0, errors: [], warnings: [], roundTrip: trips(), runs: { 'css@1280@1': { max, worst: 'a', missing: 0, fontOk: true, bad: max > 0.05 ? ['a'] : [] } }, twVsCss: { max: 0, worst: 'a' }, ...extra });
const exit = (files, more = {}) => {
  const file = path.join(fs.mkdtempSync(path.join(os.tmpdir(), 'gate-')), 'results.json');
  fs.writeFileSync(file, JSON.stringify({ seeds: 10, geometry: { files }, ...sections, ...more }));
  const r = spawnSync('node', [gate, file], { encoding: 'utf8' });
  return { code: r.status, out: r.stdout };
};
/** One passing probe, in each form, for every fixture the paint oracle must cover. */
const probesFor = (names) => names.flatMap((n) => ['css', 'tailwind'].map((form) => ({ id: `${n} ${form}: a probe`, at: [1, 1], want: 'red', got: [255, 0, 0], ok: true })));
/** A hand-written page that passes. */
const page = (name, over = {}) => ({ name, error: null, fontOk: true, anonymous: 0, nodes: 4, missing: 0, max: 0, worst: 'body', bad: [], deltas: {}, unsupported: [], approximated: [], ...over });
/** A page that fails only the way its entry in IMPORT_GAPS says. */
const gapPage = (name, over = {}) => page(name, { max: 5, worst: IMPORT_GAPS[name].nodes[0], bad: [...IMPORT_GAPS[name].nodes], unsupported: [...IMPORT_GAPS[name].codes], ...over });
const importGaps = Object.keys(IMPORT_GAPS);
const pages = () => ({ files: [...importGaps.map((g) => gapPage(g)), ...Array.from({ length: PAGE_FLOOR - importGaps.length }, (_, i) => page(`h-clean-${i}`))] });
const sections = {
  import: pages(),
  paint: { probes: probesFor(PAINT_FIXTURES), forms: PAINT_FIXTURES.map((fixture) => ({ fixture, differing: 0 })) },
  ink: { rows: [{ id: 'hug700', form: 'css', dLeft: 0, dRight: 0, dTop: 0, dBottom: 0 }], worst: 0 },
  blur: [{ blur: 8, maxAbs: 19 }],
};
const gaps = Object.keys(KNOWN_GAPS);
/** A gap that fails in the way its entry says. */
const standing = (g, over = {}) => run(g, 5, { warnings: [`${KNOWN_GAPS[g].warning}x`], runs: { 'css@1280@1': { max: 5, worst: KNOWN_GAPS[g].nodes[0], missing: 0, fontOk: true, bad: [KNOWN_GAPS[g].nodes[0]] } }, ...over });
const fz = (i, over = {}) => run(`fz-${i}`, 0, { fuzz: true, unspecified: false, g3: false, ...over });
/** A full run: the floor of fixtures, the standing gaps, and 10 seeds of which 7 are inside the contract. */
const lossName = Object.keys(IMPORT_LOSSES)[0];
const full = () => [...gaps.map((g) => standing(g)), ...Array.from({ length: FLOORS.fixtures - gaps.length }, (_, i) => (i === FLOORS.fixtures - gaps.length - 1 ? run(lossName, 0.01, { roundTrip: trips({ kindLost: IMPORT_LOSSES[lossName] }) }) : run(`f${i}`, 0.01))), ...Array.from({ length: 10 }, (_, i) => fz(i, i < 3 ? { unspecified: true, g3: true } : {}))];
const without = (files, name) => files.filter((f) => f.name !== name);
const swap = (files, name, over) => files.map((f) => (f.name === name ? { ...f, ...over } : f));

test('a full clean run passes, with every known gap still failing as stated', () => {
  assert.equal(exit(full()).code, 0);
});
test('a box off by more than 0.05 px fails', () => {
  const r = exit([...swap(full(), 'f0', { runs: { 'css@1280@1': { max: 0.06, worst: 'a', missing: 0, fontOk: true, bad: ['a'] } } })]);
  assert.equal(r.code, 1);
  assert.match(r.out, /f0: css@1280@1: box off by 0\.060/);
});
test('a node missing from the page fails', () => {
  assert.equal(exit(swap(full(), 'f0', { runs: { 'css@1280@1': { max: Infinity, worst: 'a', missing: 1, fontOk: true, bad: ['a'] } } })).code, 1);
});
test('a font that did not load fails when the file has text', () => {
  assert.equal(exit(swap(full(), 'f0', { textNodes: 2, runs: { 'css@1280@1': { max: 0, worst: 'a', missing: 0, fontOk: false, bad: [] } } })).code, 1);
});
test('tailwind that differs from css fails', () => {
  assert.equal(exit(swap(full(), 'f0', { twVsCss: { max: 0.5, worst: 'b' } })).code, 1);
});
test('a run with no fuzz seeds, too few fixtures, or too few gated seeds fails', () => {
  assert.equal(exit(full(), { seeds: 0 }).code, 1);
  assert.equal(exit(without(full(), 'f0')).code, 1);
  assert.equal(exit(full().map((f) => (f.fuzz ? { ...f, unspecified: true, g3: true } : f))).code, 1);
  assert.equal(exit(full().slice(0, -1)).code, 1, 'fewer fuzz files than seeds');
});
test('a known gap that passes fails the gate', () => {
  const r = exit(swap(full(), gaps[0], { warnings: [KNOWN_GAPS[gaps[0]].warning], runs: { 'css@1280@1': { max: 0.01, worst: 'a', missing: 0, fontOk: true, bad: [] } } }));
  assert.equal(r.code, 1);
  assert.match(r.out, /known gap closed/);
});
test('a known gap that is no longer a fixture fails the gate', () => {
  assert.equal(exit(without(full(), gaps[0])).code, 1);
});
test('a known gap must fail in the way it states: not through a font, Tailwind, another node or a missing warning', () => {
  const g = gaps[0];
  assert.equal(exit(swap(full(), g, { runs: { 'css@1280@1': { max: 5, worst: 'a', missing: 0, fontOk: false, bad: [KNOWN_GAPS[g].nodes[0]] } }, textNodes: 1 })).code, 1, 'font');
  assert.equal(exit(swap(full(), g, { twVsCss: { max: 1, worst: 'a' } })).code, 1, 'tailwind');
  assert.equal(exit(swap(full(), g, { runs: { 'css@1280@1': { max: 5, worst: 'zz', missing: 0, fontOk: true, bad: ['zz'] } } })).code, 1, 'other node');
  assert.equal(exit(swap(full(), g, { warnings: [] })).code, 1, 'warning gone');
  assert.equal(exit(swap(full(), g, { errors: ['E_X:a'] })).code, 1, 'export error');
});
test('a fuzz file outside the contract (G3) is excused its box deltas only, and its file is printed when one inside fails', () => {
  const off = { runs: { 'css@1280@1': { max: 9, worst: 'a', missing: 0, fontOk: true, bad: ['a'] } } };
  assert.equal(exit(swap(full(), 'fz-0', { ...off })).code, 0, 'G3 file with a box delta');
  assert.equal(exit(swap(full(), 'fz-0', { errors: ['E_NOT_A_FRAME:r'] })).code, 1, 'G3 file with an export error');
  assert.equal(exit(swap(full(), 'fz-0', { runs: { 'css@1280@1': { max: 0, worst: 'a', missing: 2, fontOk: true, bad: [] } } })).code, 1, 'G3 file with a missing node');
  assert.equal(exit(swap(full(), 'fz-0', { textNodes: 1, runs: { 'css@1280@1': { max: 0, worst: 'a', missing: 0, fontOk: false, bad: [] } } })).code, 1, 'G3 file with a font failure');
  const r = exit(swap(full(), 'fz-5', { ...off, nodesJson: [{ id: 'root' }] }));
  assert.equal(r.code, 1);
  assert.match(r.out, /fz-5.*\n\s+file \(add it as a fixture\): \[\{"id":"root"\}\]/);
});
test('a fuzz file that carries fill_under_hug fails: rule G1 was applied to it', () => {
  assert.equal(exit(swap(full(), 'fz-5', { fillUnderHug: true })).code, 1);
});

test('a run without the paint, ink or blur section, or with an empty one, fails; a geometry-only run is not asked for them', () => {
  assert.equal(exit(full(), { paint: undefined }).code, 1);
  assert.equal(exit(full(), { paint: { probes: [], forms: sections.paint.forms } }).code, 1);
  assert.equal(exit(full(), { paint: { probes: sections.paint.probes, forms: [] } }).code, 1);
  assert.equal(exit(full(), { ink: undefined }).code, 1);
  assert.equal(exit(full(), { ink: { rows: [], worst: 0 } }).code, 1);
  assert.equal(exit(full(), { blur: [] }).code, 1);
  assert.equal(exit(full(), { paint: undefined, ink: undefined, blur: undefined, geometryOnly: true }).code, 0);
});
test('a paint probe that fails, or two forms that draw different pixels, fail', () => {
  const probe = (ok) => ({ probes: [...sections.paint.probes, { id: 'f07-decor css: border inside: stripe', at: [43, 80], want: 'red', got: [0, 255, 0], ok }], forms: sections.paint.forms });
  assert.equal(exit(full(), { paint: probe(true) }).code, 0);
  assert.equal(exit(full(), { paint: probe(false) }).code, 1);
  const forms = sections.paint.forms.map((f) => (f.fixture === 'f07-decor' ? { ...f, differing: 12 } : f));
  assert.equal(exit(full(), { paint: { probes: sections.paint.probes, forms } }).code, 1);
});
test('the paint oracle must probe, in both forms, gradients, fills bound to a variable (default and picked mode) and no fill', () => {
  for (const name of ['f19-gradient', 'f20-variable-fill', 'f21-variable-modes', 'f22-no-fill', 'f23-fill-alpha']) {
    assert.ok(PAINT_FIXTURES.includes(name), `${name} is a required paint fixture`);
    const probes = sections.paint.probes.filter((p) => !p.id.startsWith(`${name} `));
    const r = exit(full(), { paint: { probes, forms: sections.paint.forms } });
    assert.equal(r.code, 1, `no probe of ${name}`);
    assert.match(r.out, new RegExp(`no (css|tailwind) paint probe for ${name}`));
  }
  for (const form of ['css', 'tailwind']) {
    const probes = sections.paint.probes.filter((p) => p.id !== `f22-no-fill ${form}: a probe`);
    const r = exit(full(), { paint: { probes, forms: sections.paint.forms } });
    assert.equal(r.code, 1, `f22 probed in one form only (no ${form})`);
    assert.match(r.out, new RegExp(`no ${form} paint probe for f22-no-fill`));
  }
  const forms = sections.paint.forms.filter((f) => f.fixture !== 'f22-no-fill');
  assert.equal(exit(full(), { paint: { probes: sections.paint.probes, forms } }).code, 1, 'the two forms of f22 are not compared');
});
test('text ink more than 1 px from the canvas text fails, and so does a case with no ink to compare', () => {
  const ink = (d, extra = {}) => ({ rows: [{ id: 'hug700', form: 'css', dLeft: 0, dRight: d, dTop: 0, dBottom: 0, ...extra }], worst: Math.abs(d) });
  assert.equal(exit(full(), { ink: ink(-1) }).code, 0);
  const r = exit(full(), { ink: ink(-2) });
  assert.equal(r.code, 1);
  assert.match(r.out, /text ink hug700 \(css\): an edge is 2 px/);
  const none = exit(full(), { ink: ink(0, { noInk: 'the exported page' }) });
  assert.equal(none.code, 1);
  assert.match(none.out, /no ink found on the exported page/);
});
test('a CSS blur falloff further than 20/255 from Pixi fails', () => {
  assert.equal(exit(full(), { blur: [{ blur: 8, maxAbs: 20 }] }).code, 0);
  assert.equal(exit(full(), { blur: [{ blur: 8, maxAbs: 21 }] }).code, 1);
});

test('overflowing: padding larger than the box, children larger than the content box, a zero fill', () => {
  const frame = (o) => ({ id: 'f', type: 'frame', parentId: null, layout: 'row', sw: 'fixed', sh: 'fixed', padding: 4, gap: 0, ...o });
  const kid = (o) => ({ id: 'k', type: 'rect', parentId: 'f', sw: 'fixed', sh: 'fixed', ...o });
  const rects = (f, k) => ({ f: { x: 0, y: 0, ...f }, k: { x: 0, y: 0, ...k } });
  assert.equal(overflowing([frame({}), kid({})], rects({ w: 100, h: 50 }, { w: 20, h: 20 })), false);
  assert.equal(overflowing([frame({ padding: 30 }), kid({})], rects({ w: 50, h: 50 }, { w: 1, h: 1 })), true);
  assert.equal(overflowing([frame({}), kid({})], rects({ w: 100, h: 50 }, { w: 120, h: 20 })), true);
  assert.equal(overflowing([frame({}), kid({ sw: 'fill' })], rects({ w: 100, h: 50 }, { w: 0.5, h: 20 })), true);
});

test('an export imported back with a box off by more than 0.05 px, or an imported file whose page is, fails', () => {
  assert.match(exit(swap(full(), 'f0', { roundTrip: trips({ max: 0.2, worst: 'b', bad: ['b'] }) })).out, /f0: import of r css: box off by 0\.200 px at b/);
  assert.match(exit(swap(full(), 'f0', { roundTrip: trips({ page: { max: 0.2, worst: 'imp-1', missing: 0, fontOk: true } }) })).out, /the page of the imported file is off by 0\.200 px/);
  assert.equal(exit(swap(full(), 'f0', { roundTrip: trips({ page: { max: 0, worst: 'imp-1', missing: 1, fontOk: true } }) })).code, 1, 'a node missing from the page');
  assert.equal(exit(swap(full(), 'f0', { textNodes: 1, roundTrip: trips({ page: { max: 0, worst: 'imp-1', missing: 0, fontOk: false } }) })).code, 1, 'font');
  assert.equal(exit(swap(full(), 'f0', { roundTrip: trips({ page: undefined }) })).code, 1, 'no second export');
});
test('an import that fails, refuses a property of an export, or has nothing to compare fails', () => {
  assert.match(exit(swap(full(), 'f0', { roundTrip: trips({ error: 'E_IMPORT_HTML_EMPTY' }) })).out, /f0: import of r css: E_IMPORT_HTML_EMPTY/);
  assert.match(exit(swap(full(), 'f0', { roundTrip: trips({ unsupported: ['refused_not_modelled:filter:imp-1'] }) })).out, /refused a property of an export/);
  assert.equal(exit(swap(full(), 'f0', { roundTrip: {} })).code, 1, 'no frame imported back');
  assert.equal(exit(swap(full(), 'f0', { roundTrip: trips({ missing: 2 }) })).code, 1, 'nodes of the export not in the import');
});
test('a loss of the kind of a node must be the one listed in IMPORT_LOSSES, and a listed loss that is gone fails the gate', () => {
  assert.equal(exit(swap(full(), 'f0', { roundTrip: trips({ kindLost: ['x:frame>rect'] }) })).code, 1, 'an unlisted loss');
  const r = exit(swap(full(), lossName, { roundTrip: trips() }));
  assert.equal(r.code, 1);
  assert.match(r.out, /known loss closed/);
  assert.equal(exit(without(full(), lossName)).code, 1, 'the fixture is gone');
});
test('a fixture of the known gaps is not judged on its round trip; a fuzz file under G3 is judged on boxes but has no page', () => {
  assert.equal(exit(swap(full(), gaps[0], { roundTrip: trips({ max: 9, bad: ['a'] }) })).code, 0);
  const off = { page: { max: 9, worst: 'imp-1', missing: 0, fontOk: true } };
  assert.equal(exit(swap(full(), 'fz-0', { roundTrip: trips(off) })).code, 0, 'G3 page');
  assert.equal(exit(swap(full(), 'fz-5', { roundTrip: trips(off) })).code, 1, 'a page inside the contract');
  assert.equal(exit(swap(full(), 'fz-0', { roundTrip: trips({ max: 9, bad: ['a'] }) })).code, 1, 'G3 boxes of the import');
});
test('a hand-written page off by more than 0.05 px, with no gap listed for it, fails', () => {
  const files = [...pages().files.filter((f) => f.name !== 'h-clean-0'), page('h-clean-0', { max: 0.3, worst: 'div#7', bad: ['div#7'] })];
  const r = exit(full(), { import: { files } });
  assert.equal(r.code, 1);
  assert.match(r.out, /h-clean-0: box off by 0\.300 px at div#7/);
  assert.equal(exit(full(), { import: { files: pages().files.map((f) => (f.name === 'h-clean-1' ? { ...f, error: 'E_IMPORT_HTML_EMPTY' } : f)) } }).code, 1, 'import error');
  assert.equal(exit(full(), { import: { files: pages().files.map((f) => (f.name === 'h-clean-1' ? { ...f, missing: 1 } : f)) } }).code, 1, 'no element for a source');
});
test('a known import gap that passes fails the gate', () => {
  const g = importGaps[0];
  const r = exit(full(), { import: { files: pages().files.map((f) => (f.name === g ? page(g, { unsupported: [...IMPORT_GAPS[g].codes] }) : f)) } });
  assert.equal(r.code, 1);
  assert.match(r.out, /known import gap closed/);
});
test('a known import gap must fail in the way it states: not through another node, a font or a code that is no longer listed', () => {
  const g = importGaps.find((n) => IMPORT_GAPS[n].codes.length) ?? importGaps[0];
  const swapPage = (over) => ({ import: { files: pages().files.map((f) => (f.name === g ? { ...f, ...over } : f)) } });
  assert.equal(exit(full(), swapPage({ bad: ['zz'], worst: 'zz' })).code, 1, 'another node');
  assert.equal(exit(full(), swapPage({ fontOk: false })).code, 1, 'font');
  assert.equal(exit(full(), swapPage({ unsupported: [] })).code, 1, 'the code is no longer listed');
  assert.equal(exit(full(), swapPage({ missing: 1 })).code, 1, 'a source with no element');
});
test('a run with too few hand-written pages, or without the section, or without a page a gap names, fails; a geometry-only run is not asked', () => {
  assert.equal(exit(full(), { import: { files: pages().files.slice(0, -1) } }).code, 1);
  assert.equal(exit(full(), { import: undefined }).code, 1);
  assert.equal(exit(full(), { import: { files: pages().files.filter((f) => f.name !== importGaps[0]) } }).code, 1);
  assert.equal(exit(full(), { import: undefined, paint: undefined, ink: undefined, blur: undefined, geometryOnly: true }).code, 0);
});
