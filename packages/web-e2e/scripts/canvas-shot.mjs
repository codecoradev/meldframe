// Screenshots of the canvas harness (development): node packages/web-e2e/scripts/canvas-shot.mjs <outfile> [query]
// Needs the dev server (npm run dev) and Playwright (resolved from the global module path).
import { createRequire } from 'node:module';

// Playwright is not a dependency of the package: it comes from the global module path.
const { chromium } = createRequire(import.meta.url)('playwright');

const [out = 'harness.png', query = '', base = 'http://localhost:5173'] = process.argv.slice(2);

(async () => {
  const browser = await chromium.launch({ args: ['--use-gl=angle', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1 });
  page.on('console', (m) => console.log('[page]', m.text()));
  page.on('pageerror', (e) => console.log('[pageerror]', e.message));
  await page.goto(`${base}/harness/canvas.html${query}`);
  await page.waitForSelector('[data-renderer]');
  await page.evaluate(() => document.fonts.ready);
  await page.waitForTimeout(800);
  console.log('stats', JSON.stringify(await page.evaluate(() => window.__harness.stats())));
  await page.screenshot({ path: out });
  await browser.close();
})();
