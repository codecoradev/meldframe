// Two build facts about React files, proved on throwaway builds (the product has no .tsx screen yet, and a sample
// class must not become dead CSS in it):
// 1. the React files of a package under packages/ bundle with the real vite.config.ts (react resolves from apps/web);
// 2. a Tailwind class that is written only in a .tsx file reaches the built CSS, with the same CSS entry as the
//    product (src/app.css: Tailwind, the tokens, the theme) and the same two plugins.
import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { build } from 'vite';
import { afterAll, describe, expect, it } from 'vitest';

const appCss = fileURLToPath(new URL('./app.css', import.meta.url));
const dir = mkdtempSync(join(tmpdir(), 'mf-tailwind-tsx-'));
afterAll(() => rmSync(dir, { recursive: true, force: true }));

// Tailwind reads every file under the app, this one too: the classes are put together here, so that this file never
// holds one as a whole and the product build gets no dead CSS from it.
const used = 437;
const variant = 439;
const unused = 438;
const px = (n: number) => `w-[${n}px]`;

async function builtCss(): Promise<string> {
  writeFileSync(join(dir, 'style.css'), `@import '${appCss}';\n@source '${dir}';\n`);
  writeFileSync(join(dir, 'main.tsx'), `import './style.css';\nexport const Probe = () => <p className="${px(used)} hov:${px(variant)}">probe</p>;\n`);
  const result = await build({
    root: dir,
    configFile: false,
    logLevel: 'silent',
    plugins: [react({ include: /\.tsx$/ }), tailwindcss()],
    // React itself is not needed to read the CSS: the page is built with react left out of the bundle.
    build: { write: false, rollupOptions: { input: join(dir, 'main.tsx'), external: [/^react/] } },
  });
  const outputs = (Array.isArray(result) ? result : [result]).flatMap((r) => ('output' in r ? r.output : []));
  return outputs.flatMap((o) => (o.type === 'asset' && o.fileName.endsWith('.css') ? [String(o.source)] : [])).join('\n');
}

describe('React files of the packages in the product build', () => {
  it('bundles the hook and the island helper, with react resolved from apps/web (the packages have no node_modules)', { timeout: 60_000 }, async () => {
    const kit = fileURLToPath(new URL('../../../packages/web-chrome/src/react', import.meta.url));
    writeFileSync(join(dir, 'kit.ts'), `import { island } from '${kit}/island';\nimport { useAppState } from '${kit}/useAppState';\nconsole.log(island, useAppState);\n`);
    const result = await build({
      configFile: fileURLToPath(new URL('../vite.config.ts', import.meta.url)),
      logLevel: 'silent',
      build: { write: false, rollupOptions: { input: join(dir, 'kit.ts') } },
    });
    const outputs = (Array.isArray(result) ? result : [result]).flatMap((r) => ('output' in r ? r.output : []));
    const js = outputs.flatMap((o) => (o.type === 'chunk' ? [o.code] : [])).join('\n');
    expect(js).toContain('createRoot');
    expect(js).toContain('useSyncExternalStore');
  });
});

describe('Tailwind and .tsx', () => {
  it('emits the classes that only a .tsx file uses, and only those', { timeout: 60_000 }, async () => {
    const css = await builtCss();
    expect(css).toContain(`.w-\\[${used}px\\]`);
    expect(css).toContain(`.hov\\:w-\\[${variant}px\\]`); // the project's own variant from tailwind.css, in a class of the .tsx
    expect(css).not.toContain(`${unused}px`); // a class that no file writes is not there
  });
});
