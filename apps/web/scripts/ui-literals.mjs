// Usage: node scripts/ui-literals.mjs  (from apps/web). Exits non-zero on any literal outside the tokens module.
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { findLiterals, isExempt } from './ui-literals-lib.mjs';

function* walk(dir) {
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, e.name);
    if (e.isDirectory()) {
      if (e.name !== 'wasm') yield* walk(p);
    } else if (/\.(?:ts|svelte|css|scss|html|cjs|mjs)$/.test(e.name)) yield p;
  }
}

const found = [];
for (const root of ['src', 'dev', 'e2e', 'index.html', 'vite.config.ts']) {
  const files = /\.(?:html|ts)$/.test(root) ? [root] : [...walk(root)];
  for (const f of files) if (!isExempt(f)) found.push(...findLiterals(f, readFileSync(f, 'utf8')));
}
for (const f of found) console.error(`${f.path}:${f.line}: ${f.kind} literal; read it from src/tokens.ts`);
if (found.length) process.exit(1);
console.log('ui-literals (web): ok');
