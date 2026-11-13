// Compile the Tailwind utilities used by an exported page with the Tailwind v4 compiler from the spike's node_modules.
import { createRequire } from 'node:module';
import { SPIKE } from './lib.mjs';
const req = createRequire(SPIKE + '/node_modules/');
const { compile } = await import(SPIKE + '/node_modules/tailwindcss/dist/lib.mjs');
export async function compileTailwind(html) {
  const cands = new Set();
  for (const m of html.matchAll(/class="([^"]*)"/g)) for (const c of m[1].split(/\s+/)) if (c) cands.add(c);
  const c = await compile('@tailwind utilities;');
  return c.build([...cands]);
}
