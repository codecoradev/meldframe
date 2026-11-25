// Made-up results of the importer's oracle for the tests of the gate: a clean run, with every known gap failing in the way its entry says.
import { IMPORT_GAPS } from '../known-gaps.mjs';
import { IMPORT_FLOORS } from '../gate-import.mjs';

export const page = (name, over = {}) => ({ name, error: null, compared: 3, missing: [], ambiguous: [], max: 0, worst: null, bad: [], detail: [], fontOk: true, textNodes: 0, nodes: 3, findings: [], approximated: [], pixels: { differing: 0, total: 100 }, ...over });
export const trip = (name, over = {}) => ({ name, frame: 'r', form: 'css', fuzz: false, error: null, nodes: 3, original: 3, max: 0, worst: null, diffs: [], unsupported: ['::before:refused_generated_content', 'font-family:refused_not_modelled'], fixedPoint: null, skipped: false, ...over });
/** A page that fails in the way its gap says. */
export const standingPage = (name) => {
  const g = IMPORT_GAPS[name];
  return page(name, {
    max: g.nodes.length ? 5 : 0, worst: g.nodes[0] ?? null, bad: g.nodes, detail: g.nodes.map((n) => ({ name: n, browser: [0, 0, 1, 1], core: [0, 0, 2, 2] })),
    findings: g.warning ? [`${g.warning}x`] : [], pixels: { differing: g.pixels ? 9 : 0, total: 100 },
  });
};
export const standingTrip = (name) => trip(name, { max: 5, worst: IMPORT_GAPS[name].nodes[0] });

const gapsOf = (kind) => Object.keys(IMPORT_GAPS).filter((n) => IMPORT_GAPS[n].kind === kind);
/** A full clean run of the importer's oracle. */
export function importSection() {
  const pages = [...gapsOf('page').map(standingPage), ...Array.from({ length: IMPORT_FLOORS.pages }, (_, i) => page(`h${i}`))];
  const roundTrips = [...gapsOf('roundtrip').map(standingTrip), ...Array.from({ length: IMPORT_FLOORS.roundTrips }, (_, i) => trip(`f${i}`))];
  return { pages, roundTrips };
}
