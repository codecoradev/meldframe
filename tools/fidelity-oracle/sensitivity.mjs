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

const both = (e, f) => ({ ...e, html: f(e.html), css: f(e.css) });

/** Mistakes the pixel probes must catch: an image fit and a shadow offset that are wrong in both forms, and one in each of gradients,
 *  fills bound to a variable (the variable, and the mode picked, a radius and an opacity) and no fill. */
export const PAINT_MUTATIONS = {
  'image cover becomes contain': (form, e) => ({ ...e, html: e.html.replaceAll('background-size:cover', 'background-size:contain'), css: e.css.replaceAll('background-size:cover', 'background-size:contain') }),
  'a linear gradient at 90deg is written at 270deg': (form, e) => both(e, (t) => t.replaceAll('linear-gradient(90deg', 'linear-gradient(270deg')),
  'a linear gradient at 45deg is written at 90deg': (form, e) => both(e, (t) => t.replaceAll('linear-gradient(45deg', 'linear-gradient(90deg')),
  'a linear gradient at 45deg is written at 135deg': (form, e) => both(e, (t) => t.replaceAll('linear-gradient(45deg', 'linear-gradient(135deg')),
  'a linear gradient at 45deg is written at 0deg': (form, e) => both(e, (t) => t.replaceAll('linear-gradient(45deg', 'linear-gradient(0deg')),
  'a linear gradient at 0deg is written at 180deg': (form, e) => both(e, (t) => t.replaceAll('linear-gradient(0deg', 'linear-gradient(180deg')),
  'a linear gradient at 180deg is written at 0deg': (form, e) => both(e, (t) => t.replaceAll('linear-gradient(180deg', 'linear-gradient(0deg')),
  'a radial gradient becomes an ellipse': (form, e) => both(e, (t) => t.replace(/circle(_| )farthest-corner/g, 'ellipse$1farthest-corner')),
  'a bound fill points at another variable': (form, e) => both(e, (t) => t.replaceAll('var(--brand-color-primary)', 'var(--brand-color-surface)')),
  'the mode picked for a collection is not written': (form, e) => both(e, (t) => t.replaceAll(' data-mode-brand="dark"', '')),
  'a bound radius is left out': (form, e) => both(e, (t) => t.replace(';border-radius:calc(var(--brand-radius-round) * 1px)', '').replace(' rounded-[calc(var(--brand-radius-round)_*_1px)]', '')),
  'a bound opacity is left out': (form, e) => both(e, (t) => t.replace(';opacity:var(--brand-opacity-muted)', '').replace(' opacity-[var(--brand-opacity-muted)]', '')),
  'a fill of none paints the default fill': (form, e) => both(e, (t) => t.replace('[data-node="n1"]{', '[data-node="n1"]{background-color:#3f3f46;').replace('data-node="n1" data-name="n1" class="', 'data-node="n1" data-name="n1" class="bg-[color:#3f3f46] ')),
  'the alpha of a fill is dropped': (form, e) => both(e, (t) => t.replace(/(background-color:|bg-\[color:)(#[0-9a-f]{6})[0-9a-f]{2}/g, '$1$2')),
  'a hard shadow moves 10 px': (form, e) => both(e, (t) => t.replace(/20px[ _]0px[ _]0px[ _]0px[ _]#ff0000ff/g, (m) => m.replace('20px', '30px'))),
};

/** A mutation that changed nothing in a form is not a test of the gate: its search string was not in that export. `tracked` wraps a
 *  mutation and `missed()` names the forms in which no export of the run was changed by it. */
export function tracked(mutate) {
  const hit = { css: false, tailwind: false };
  return {
    mutate: (form, e) => {
      const m = mutate(form, e);
      if (m.html !== e.html || m.css !== e.css) hit[form] = true;
      return m;
    },
    missed: () => Object.keys(hit).filter((f) => !hit[f]),
  };
}

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
      const t = tracked(mutate);
      const results = { seeds: 1, only: true, paint: await paint({ browser, base, mutate: t.mutate }) };
      const failures = evaluate(results);
      const nothing = t.missed();
      console.log(`${failures.length && !nothing.length ? 'caught ' : 'MISSED '} "${name}": ${failures.length} violation(s)${nothing.length ? `, but its search string is not in the ${nothing.join(' and ')} export` : ''}${failures[0] ? `, first: ${failures[0]}` : ''}`);
      if (!failures.length || nothing.length) bad++;
    }
    for (const [name, mutate] of Object.entries(MUTATIONS)) {
      const t = tracked(mutate);
      const failures = await check({ browser, base, mp, mutate: t.mutate });
      const nothing = t.missed();
      console.log(`${failures.length && !nothing.length ? 'caught ' : 'MISSED '} "${name}": ${failures.length} violation(s)${nothing.length ? `, but its search string is not in the ${nothing.join(' and ')} export` : ''}${failures[0] ? `, first: ${failures[0].split('\n')[0]}` : ''}`);
      if (!failures.length || nothing.length) bad++;
    }
  } finally {
    await browser.close();
    close();
  }
  process.exit(bad ? 1 : 0);
}
