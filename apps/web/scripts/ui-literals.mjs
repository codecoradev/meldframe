// Usage: node scripts/ui-literals.mjs  (from apps/web). Exits non-zero on any literal outside the tokens module.
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { findLiterals, isExempt } from './ui-literals-lib.mjs';

function* walk(dir) {
  if (!existsSync(dir)) return;
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, e.name);
    if (e.isDirectory()) {
      if (!['wasm', 'node_modules', 'dist'].includes(e.name)) yield* walk(p);
    } else if (/\.(?:ts|tsx|mts|js|cjs|mjs|svelte|css|scss|html|svg|json)$/.test(e.name)) yield p;
  }
}

const found = [];
for (const root of ['src', 'dev', 'index.html', 'vite.config.ts']) {
  const files = /\.(?:html|ts)$/.test(root) ? [root] : [...walk(root)];
  for (const f of files) if (existsSync(f) && !isExempt(f)) found.push(...findLiterals(f, readFileSync(f, 'utf8')));
}
for (const f of found) console.error(`${f.path}:${f.line}: ${f.kind} literal; read it from src/tokens.ts`);
if (found.length) process.exit(1);
console.log('ui-literals (web): ok');
