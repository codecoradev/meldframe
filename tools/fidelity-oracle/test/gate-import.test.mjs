// The importer's gate on made-up results: a violation and a known gap that closed fail it, a clean run and a standing gap pass it.
import test from 'node:test';
import assert from 'node:assert/strict';
import { IMPORT_GAPS } from '../known-gaps.mjs';
import { evaluateImport, IMPORT_FLOORS } from '../gate-import.mjs';
import { importSection, page, standingPage, standingTrip, trip } from './import-results.mjs';

const results = (imp = importSection(), more = {}) => ({ seeds: 10, geometry: { files: [{ name: 'f17-fill-under-hug' }] }, import: imp, ...more });
const gapPage = Object.keys(IMPORT_GAPS).find((n) => IMPORT_GAPS[n].kind === 'page' && IMPORT_GAPS[n].warning && IMPORT_GAPS[n].nodes.length);
const pixelGap = Object.keys(IMPORT_GAPS).find((n) => IMPORT_GAPS[n].pixels);
const swapPage = (name, over) => { const s = importSection(); s.pages = s.pages.map((p) => (p.name === name ? { ...p, ...over } : p)); return s; };
const swapTrip = (name, over) => { const s = importSection(); s.roundTrips = s.roundTrips.map((r) => (r.name === name ? { ...r, ...over } : r)); return s; };
const text = (r) => evaluateImport(r).join('\n');

test('a full clean run passes, with every known gap failing as stated', () => {
  assert.deepEqual(evaluateImport(results()), []);
});
test('a run with no import section fails', () => {
  assert.equal(evaluateImport({ seeds: 10, geometry: { files: [] } }).length, 1);
});
test('a page that differs from the browser fails: a box, a refusal, a missing or doubled name, a font, pixels', () => {
  const box = { max: 0.06, worst: 'a', bad: ['a'], detail: [{ name: 'a', browser: [0, 0, 1, 1], core: [0.06, 0, 1, 1] }] };
  assert.match(text(results(swapPage('h0', box))), /h0: box of a off by 0\.060/);
  assert.match(text(results(swapPage('h0', { error: 'E_IMPORT_HTML_EMPTY: x' }))), /h0: the importer refused the page/);
  assert.match(text(results(swapPage('h0', { missing: ['q'] }))), /no imported node is named q/);
  assert.match(text(results(swapPage('h0', { ambiguous: ['q'] }))), /more than one imported node is named q/);
  assert.match(text(results(swapPage('h0', { fontOk: false, textNodes: 2 }))), /font did not load/);
  assert.match(text(results(swapPage('h0', { pixels: { differing: 1, total: 100 } }))), /1 of 100 pixels differ/);
});
test('a box exactly at the limit passes', () => {
  assert.deepEqual(evaluateImport(results(swapPage('h0', { max: 0.05, worst: 'a' }))), []);
});
test('a known gap page that passes fails the gate', () => {
  const s = swapPage(gapPage, { max: 0, bad: [], detail: [], worst: null, findings: [`${IMPORT_GAPS[gapPage].warning}x`] });
  assert.match(text(results(s)), new RegExp(`${gapPage}: known import gap closed`));
});
test('a known gap page must fail in the way it states: not another node, another way, or with its warning gone', () => {
  const g = IMPORT_GAPS[gapPage];
  assert.match(text(results(swapPage(gapPage, { bad: [...g.nodes, 'zz'] }))), /node zz is off and is not part of the stated cause/);
  assert.match(text(results(swapPage(gapPage, { fontOk: false, textNodes: 1 }))), /fails another way: the font did not load/);
  assert.match(text(results(swapPage(gapPage, { missing: ['zz'] }))), /fails another way/);
  assert.match(text(results(swapPage(gapPage, { error: 'E_X: x' }))), /fails another way/);
  assert.match(text(results(swapPage(gapPage, { pixels: { differing: 3, total: 9 } }))), /fails another way: 3 of 9 pixels/);
  assert.match(text(results(swapPage(gapPage, { findings: [] }))), /no longer reports/);
});
test('a gap that states pixels closes when they stop differing', () => {
  assert.match(text(results(swapPage(pixelGap, { pixels: { differing: 0, total: 100 } }))), /known import gap closed/);
  assert.deepEqual(evaluateImport(results(swapPage(pixelGap, {}))), []);
});
test('a known gap that is no longer a page or a fixture fails the gate', () => {
  const s = importSection();
  s.pages = s.pages.filter((p) => p.name !== gapPage);
  assert.match(text(results(s)), new RegExp(`known import gap ${gapPage} is not a page any more`));
  assert.match(text(results(importSection(), { geometry: { files: [] } })), /f17-fill-under-hug is not a fixture any more/);
});
test('a round trip that differs fails: an error, a count, a field, a box, a fixed point, a refusal the export does not carry', () => {
  assert.match(text(results(swapTrip('f0', { error: 'E_X: y' }))), /f0 r \(css\): the importer refused the export/);
  assert.match(text(results(swapTrip('f0', { nodes: 2 }))), /3 nodes became 2/);
  assert.match(text(results(swapTrip('f0', { diffs: ['a radius: 4 became 5'] }))), /a radius: 4 became 5/);
  assert.match(text(results(swapTrip('f0', { max: 0.2, worst: 'b' }))), /box of b off by 0\.200 px after the round trip/);
  assert.match(text(results(swapTrip('f0', { fixedPoint: 'at byte 4' }))), /not a fixed point/);
  assert.match(text(results(swapTrip('f0', { unsupported: ['transform:refused_not_modelled'] }))), /refused transform:refused_not_modelled on an export/);
  assert.deepEqual(evaluateImport(results(swapTrip('f0', { skipped: true, max: 9, diffs: ['x'] }))).filter((l) => l.startsWith('f0')), [], 'a skipped fixture is not judged');
});
test('the round trip of a known gap fixture must fail as stated, and closes when it stops failing', () => {
  const name = Object.keys(IMPORT_GAPS).find((n) => IMPORT_GAPS[n].kind === 'roundtrip');
  assert.match(text(results(swapTrip(name, { max: 0, worst: null }))), /known import gap closed/);
  assert.match(text(results(swapTrip(name, { worst: 'zz' }))), /node zz is off and is not part of the stated cause/);
  assert.match(text(results(swapTrip(name, { diffs: ['x fill: a became b'] }))), /fails another way/);
  assert.deepEqual(evaluateImport(results(swapTrip(name, standingTrip(name)))), []);
});
test('a run that checks less than the floors fails', () => {
  const s = importSection();
  s.pages = s.pages.slice(0, IMPORT_FLOORS.gapPages);
  assert.match(text(results(s)), /pages ran outside the known gaps/);
  const t = importSection();
  t.roundTrips = t.roundTrips.slice(0, 5);
  assert.match(text(results(t)), /round trips ran/);
  const u = importSection();
  u.pages = u.pages.map((p) => ({ ...p, pixels: undefined }));
  assert.match(text(results(u)), /pixels of fewer than/);
});
test('the helpers build what they say', () => {
  assert.equal(standingPage(gapPage).bad.length, IMPORT_GAPS[gapPage].nodes.length);
  assert.equal(page('x').max, 0);
  assert.equal(trip('y').skipped, false);
});
