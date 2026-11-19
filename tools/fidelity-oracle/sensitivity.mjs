// The gate can fail: loads the exports with a deliberate mistake in them and requires the gate to report it. Without the
// mistake the same run must pass. `node sensitivity.mjs`; exit 1 when a mutation goes unnoticed. The mistakes are the ones the
// contract exists to catch: an alignment that overflows differently, a gap that is a fraction of a pixel off, a text line
// height that is a pixel off.
import { serve, launch, openMeasurePage } from './lib.mjs';
import { geometry } from './geometry.mjs';
import { paint } from './probes.mjs';
import { evaluate } from './gate.mjs';

export const MUTATIONS = {
  'safe center becomes center': (form, e) => ({ ...e, html: e.html.replaceAll('safe_center', 'center'), css: e.css.replaceAll('safe center', 'center') }),
  'every gap is 0.2 px wider': (form, e) => {
    const more = (v) => String(+v + 0.2);
    return form === 'css'
      ? { ...e, css: e.css.replace(/gap:(\d+(?:\.\d+)?)px/g, (_, v) => `gap:${more(v)}px`), html: e.html.replace(/gap:(\d+(?:\.\d+)?)px/g, (_, v) => `gap:${more(v)}px`) }
      : { ...e, html: e.html.replace(/gap-\[(\d+(?:\.\d+)?)px\]/g, (_, v) => `gap-[${more(v)}px]`) };
  },
  'every text line is 1 px taller': (form, e) => ({
    ...e,
    html: e.html.replace(/line-height:(\d+(?:\.\d+)?)px/g, (_, v) => `line-height:${+v + 1}px`).replace(/leading-\[(\d+(?:\.\d+)?)px\]/g, (_, v) => `leading-[${+v + 1}px]`),
    css: e.css.replace(/line-height:(\d+(?:\.\d+)?)px/g, (_, v) => `line-height:${+v + 1}px`),
  }),
};

/** Mistakes the pixel probes must catch: an image fit and a shadow offset that are wrong in both forms. */
export const PAINT_MUTATIONS = {
  'image cover becomes contain': (form, e) => ({ ...e, html: e.html.replaceAll('background-size:cover', 'background-size:contain'), css: e.css.replaceAll('background-size:cover', 'background-size:contain') }),
  'a hard shadow moves 10 px': (form, e) => ({ ...e, html: e.html.replace(/20px 0px 0px 0px #ff0000ff/g, '30px 0px 0px 0px #ff0000ff'), css: e.css.replace(/20px 0px 0px 0px #ff0000ff/g, '30px 0px 0px 0px #ff0000ff') }),
};

/** The failures the gate reports for a run with `mutate` (none for the identity). Returns lines. */
export async function check({ browser, base, mp, mutate, seeds = 10 }) {
  const results = { seeds, geometryOnly: true, geometry: await geometry({ browser, base, mp, seeds, mutate }) };
  return evaluate(results);
}

if (import.meta.url === (await import('node:url')).pathToFileURL(process.argv[1]).href) {
  const { base, close } = await serve();
  const browser = await launch();
  let bad = 0;
  try {
    const mp = await openMeasurePage(browser, base);
    const baseline = await check({ browser, base, mp, mutate: (_f, e) => e });
    console.log(baseline.length ? `FAIL: the unmutated run fails:\n${baseline.join('\n')}` : 'unmutated run: passes');
    bad += baseline.length ? 1 : 0;
    for (const [name, mutate] of Object.entries(PAINT_MUTATIONS)) {
      const results = { seeds: 1, only: true, paint: await paint({ browser, base, mutate }) };
      const failures = evaluate(results);
      console.log(`${failures.length ? 'caught ' : 'MISSED '} "${name}": ${failures.length} violation(s)${failures[0] ? `, first: ${failures[0]}` : ''}`);
      if (!failures.length) bad++;
    }
    for (const [name, mutate] of Object.entries(MUTATIONS)) {
      const failures = await check({ browser, base, mp, mutate });
      console.log(`${failures.length ? 'caught ' : 'MISSED '} "${name}": ${failures.length} violation(s)${failures[0] ? `, first: ${failures[0].split('\n')[0]}` : ''}`);
      if (!failures.length) bad++;
    }
  } finally {
    await browser.close();
    close();
  }
  process.exit(bad ? 1 : 0);
}
