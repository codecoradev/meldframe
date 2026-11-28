// Records the table of text measurements for the Rune measurer conformance run (E8.2.3, C2 of docs/spikes/rune-desktop-spike.md):
// the answers Chromium gives with Pixi `CanvasTextMetrics` and Inter to every question the layout core asks about the text
// fixtures of `crates/core/meldframe-layout/tests/fixtures/oracle_texts.json`, and to the export's `line-height` question. It is
// the reference measurer of the suite, so the numbers come from the browser and are never edited by hand.
//
//   cargo build -p meldframe-oracle-runner      (the runner that says which questions the table still lacks)
//   node record-text-table.mjs [out.json]       default: frontends/rune/tools/measure/data/chromium_text_table.json
//
// Linux only, like the oracle (platform.mjs). The file names the browser it was recorded with.
import fs from 'node:fs';
import path from 'node:path';
import { serve, launch, openMeasurePage, solve, ROOT } from './lib.mjs';

const TEXTS = path.join(ROOT, 'crates/core/meldframe-layout/tests/fixtures/oracle_texts.json');
const OUT = process.argv[2] ?? path.join(ROOT, 'frontends/rune/tools/measure/data/chromium_text_table.json');
const pkg = (name) => JSON.parse(fs.readFileSync(path.join(path.dirname(new URL(import.meta.url).pathname), 'node_modules', name, 'package.json'), 'utf8')).version;

const fixtures = JSON.parse(fs.readFileSync(TEXTS, 'utf8')).fixtures.map(({ name, nodes }) => ({ name, nodes }));
const { base, close } = await serve();
const browser = await launch();
try {
  const mp = await openMeasurePage(browser, base);
  // The weight question of the measurer run: one text at the nine weights of the variable face, which no fixture asks together.
  const table = new Map();
  const weights = [100, 200, 300, 400, 500, 600, 700, 800, 900].map((fontWeight) => ({ text: 'Weight reaches the width', fontSize: 16, fontWeight, wrapW: null }));
  for (const e of await mp.measure(weights)) table.set(JSON.stringify([e.text, e.fontSize, e.fontWeight, e.wrapW]), e);
  const { rounds } = await solve(fixtures, mp, table);
  const entries = [...table.values()]
    .map(({ text, fontSize, fontWeight, wrapW, w, h }) => ({ text, fontSize, fontWeight, wrapW, w, h }))
    .sort((a, b) => JSON.stringify([a.text, a.fontSize, a.fontWeight, a.wrapW]).localeCompare(JSON.stringify([b.text, b.fontSize, b.fontWeight, b.wrapW])));
  const recordedWith = `Chromium ${await browser.version()} on ${process.platform}; pixi.js ${pkg('pixi.js')}; Inter Variable (@fontsource-variable/inter ${pkg('@fontsource-variable/inter')}); ${fixtures.length} fixtures, ${rounds} rounds`;
  fs.mkdirSync(path.dirname(OUT), { recursive: true });
  fs.writeFileSync(OUT, `{"recorded_with":${JSON.stringify(recordedWith)},"entries":[\n${entries.map((e) => `  ${JSON.stringify(e)}`).join(',\n')}\n]}\n`);
  console.log(`${entries.length} measurements, ${recordedWith}\nwritten to ${OUT}`);
} finally {
  await browser.close();
  close();
}
